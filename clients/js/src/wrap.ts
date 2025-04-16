import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  containsBytes,
  createTransactionMessage,
  fetchEncodedAccount,
  GetAccountInfoApi,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  SignatureBytes,
  Transaction,
  TransactionSigner,
  TransactionWithBlockhashLifetime,
} from '@solana/kit';
import { findAssociatedTokenPda, getTokenDecoder } from '@solana-program/token-2022';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getWrapInstruction,
  WrapInput,
} from './generated';
import { Blockhash } from '@solana/rpc-types';
import { getOwnerFromAccount } from './utilities';

const getMintFromTokenAccount = async (
  rpc: Rpc<GetAccountInfoApi>,
  tokenAccountAddress: Address,
): Promise<Address> => {
  const account = await fetchEncodedAccount(rpc, tokenAccountAddress);
  if (!account.exists) {
    throw new Error(`Unwrapped token account ${tokenAccountAddress} not found.`);
  }
  return getTokenDecoder().decode(account.data).mint;
};

interface TxBuilderArgs {
  payer: TransactionSigner;
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  wrappedMint: Address;
  wrappedMintAuthority: Address;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  transferAuthority: Address | TransactionSigner;
  unwrappedMint: Address;
  recipientWrappedTokenAccount: Address;
  unwrappedTokenProgram: Address;
  multiSigners?: TransactionSigner[];
}

interface TxBuilderArgsWithMultiSigners extends TxBuilderArgs {
  multiSigners: TransactionSigner[];
}

// Used to collect signatures
export const multisigOfflineSignWrapTx = async (args: TxBuilderArgsWithMultiSigners) => {
  return buildWrapTransaction(args);
};

const messageBytesEqual = (
  results: (Transaction & TransactionWithBlockhashLifetime)[],
): boolean => {
  // If array has only one element, return true
  if (results.length === 1) {
    return true;
  }

  // Use the first result as reference
  const reference = results[0];
  if (!reference) throw new Error('No transactions in input');

  // Compare each result with the reference
  for (const current of results) {
    const sameLength = reference.messageBytes.length === current.messageBytes.length;
    const sameBytes = containsBytes(reference.messageBytes, current.messageBytes, 0);

    if (!sameLength || !sameBytes) {
      return false;
    }
  }

  return true;
};

const combineSignatures = (signedTxs: (Transaction & TransactionWithBlockhashLifetime)[]) => {
  // Step 1: Determine the canonical signer order from the first signed transaction.
  //         Insertion order is the way to re-create this. Without it, verification will fail.
  const firstSignedTx = signedTxs[0];
  if (!firstSignedTx) {
    throw new Error('No signed transactions provided');
  }

  const allSignatures: Record<string, SignatureBytes | null> = {};

  // Step 1: Insert a null signature for each signer, maintaining the order of the signatures from the first signed transaction
  for (const pubkey of Object.keys(firstSignedTx.signatures)) {
    allSignatures[pubkey] = null;
  }

  // Step 2: Gather all signatures from all transactions
  for (const signedTx of signedTxs) {
    for (const [address, signature] of Object.entries(signedTx.signatures)) {
      if (signature) {
        // only store non-null signers
        allSignatures[address] = signature;
      }
    }
  }

  // Step 3: Assert all signatures are set
  const missingSigners: string[] = [];
  for (const [pubkey, signature] of Object.entries(allSignatures)) {
    if (signature === null) {
      missingSigners.push(pubkey);
    }
  }
  if (missingSigners.length > 0) {
    throw new Error(`Missing signatures for: ${missingSigners.join(', ')}`);
  }

  return allSignatures;
};

interface MultiSigBroadcastArgs {
  signedTxs: (Transaction & TransactionWithBlockhashLifetime)[];
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
}

// Combines, validates, and broadcasts outputs of multisigOfflineSignWrap()
export const combinedMultisigWrapTx = async ({ signedTxs, blockhash }: MultiSigBroadcastArgs) => {
  const messagesEqual = messageBytesEqual(signedTxs);
  if (!messagesEqual) throw new Error('Messages are not all the same');
  if (!signedTxs[0]) throw new Error('No signed transactions provided');

  const tx = {
    messageBytes: signedTxs[0].messageBytes,
    signatures: combineSignatures(signedTxs),
    lifetimeConstraint: blockhash,
  };

  assertTransactionIsFullySigned(tx);

  return tx;
};

interface SingleSignerWrapArgs {
  rpc: Rpc<GetAccountInfoApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  payer: TransactionSigner; // Fee payer and default transfer authority
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  transferAuthority?: Address | TransactionSigner; // Defaults to payer if not provided
  unwrappedMint?: Address; // Will fetch from unwrappedTokenAccount if not provided
  recipientWrappedTokenAccount?: Address; // Defaults to payer's ATA if not provided
  unwrappedTokenProgram?: Address; // Will fetch from unwrappedTokenAccount owner if not provided
}

export const singleSignerWrapTx = async ({
  rpc,
  blockhash,
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  recipientWrappedTokenAccount: inputRecipientTokenAccount,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerWrapArgs) => {
  const {
    unwrappedMint,
    unwrappedTokenProgram,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
    transferAuthority,
  } = await resolveAddrs({
    rpc,
    payer,
    inputTransferAuthority,
    inputUnwrappedMint,
    unwrappedTokenAccount,
    inputUnwrappedTokenProgram,
    wrappedTokenProgram,
    inputRecipientTokenAccount,
  });

  const tx = await buildWrapTransaction({
    blockhash,
    payer,
    unwrappedTokenAccount,
    escrowAccount,
    wrappedTokenProgram,
    amount,
    transferAuthority,
    unwrappedMint,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
    unwrappedTokenProgram,
  });

  return {
    tx,
    recipientWrappedTokenAccount,
    escrowAccount,
    amount: BigInt(amount),
  };
};

// Meant to handle all of the potential default values
const resolveAddrs = async ({
  rpc,
  payer,
  unwrappedTokenAccount,
  wrappedTokenProgram,
  inputTransferAuthority,
  inputUnwrappedMint,
  inputRecipientTokenAccount,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<GetAccountInfoApi>;
  payer: TransactionSigner;
  unwrappedTokenAccount: Address;
  wrappedTokenProgram: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputUnwrappedMint?: Address;
  inputRecipientTokenAccount?: Address;
  inputUnwrappedTokenProgram?: Address;
}) => {
  const unwrappedMint =
    inputUnwrappedMint ?? (await getMintFromTokenAccount(rpc, unwrappedTokenAccount));
  const unwrappedTokenProgram =
    inputUnwrappedTokenProgram ?? (await getOwnerFromAccount(rpc, unwrappedTokenAccount));
  const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
  const recipientWrappedTokenAccount =
    inputRecipientTokenAccount ??
    (
      await findAssociatedTokenPda({
        owner: payer.address,
        mint: wrappedMint,
        tokenProgram: wrappedTokenProgram,
      })
    )[0];

  const transferAuthority = inputTransferAuthority ?? payer;

  return {
    transferAuthority,
    unwrappedMint,
    unwrappedTokenProgram,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
  };
};

const buildWrapTransaction = async ({
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority,
  unwrappedMint,
  recipientWrappedTokenAccount,
  unwrappedTokenProgram,
  wrappedMint,
  wrappedMintAuthority,
  blockhash,
  multiSigners = [],
}: TxBuilderArgs) => {
  const wrapInstructionInput: WrapInput = {
    recipientWrappedTokenAccount,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    wrappedTokenProgram,
    unwrappedTokenAccount,
    unwrappedMint,
    unwrappedEscrow: escrowAccount,
    transferAuthority,
    amount: BigInt(amount),
    multiSigners,
  };

  const wrapInstruction = getWrapInstruction(wrapInstructionInput);

  return pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([wrapInstruction], tx),
  );
};
