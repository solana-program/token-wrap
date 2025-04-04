import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  createTransactionMessage,
  fetchEncodedAccount,
  getSignatureFromTransaction,
  KeyPairSigner,
  partiallySignTransactionMessageWithSigners,
  pipe,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayer,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  SignatureBytes,
  SolanaRpcApi,
  SolanaRpcSubscriptionsApi,
  TransactionSigner,
} from '@solana/kit';
import { findAssociatedTokenPda, getTokenDecoder } from '@solana-program/token-2022';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getWrapInstruction,
  WrapInput,
} from './generated';
import { Blockhash } from '@solana/rpc-types';

const getMintFromTokenAccount = async (
  rpc: Rpc<SolanaRpcApi>,
  tokenAccountAddress: Address,
): Promise<Address> => {
  const account = await fetchEncodedAccount(rpc, tokenAccountAddress);
  if (!account.exists) {
    throw new Error(`Unwrapped token account ${tokenAccountAddress} not found.`);
  }
  return getTokenDecoder().decode(account.data).mint;
};

export const getOwnerFromAccount = async (
  rpc: Rpc<SolanaRpcApi>,
  accountAddress: Address,
): Promise<Address> => {
  const accountInfo = await rpc.getAccountInfo(accountAddress, { encoding: 'base64' }).send();
  if (!accountInfo.value) {
    throw new Error(`Account ${accountAddress} not found.`);
  }
  return accountInfo.value.owner;
};

interface TxBuilderArgs {
  payer: KeyPairSigner | Address;
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

// Used to collect signatures
export const multisigOfflineSignWrap = async (args: Required<TxBuilderArgs>) => {
  return buildWrapTransaction(args);
};

const messageBytesEqual = (
  results: Awaited<ReturnType<typeof multisigOfflineSignWrap>>[],
): boolean => {
  // If array has only one element, return true
  if (results.length === 1) {
    return true;
  }

  // Use the first result as reference
  const reference = results[0];
  if (!reference) throw new Error('No transactions in input');

  // Compare each result with the reference
  for (let i = 1; i < results.length; i++) {
    const current = results[i];
    if (!current) throw new Error('Nullish entry in signature results array');

    // Compare messageBytes
    if (reference.messageBytes.length !== current.messageBytes.length) {
      return false;
    }

    for (let j = 0; j < reference.messageBytes.length; j++) {
      if (reference.messageBytes[j] !== current.messageBytes[j]) {
        return false;
      }
    }
  }

  return true;
};

const combineSignatures = (signedTxs: Awaited<ReturnType<typeof multisigOfflineSignWrap>>[]) => {
  // Step 1: Determine the canonical signer order from the first signed transaction.
  //         Insertion order is the way to re-create this. Without it, verification will fail.
  const firstSignedTx = signedTxs[0];
  if (!firstSignedTx) {
    throw new Error('No signed transactions provided');
  }

  const signerOrder: string[] = [];
  const allSignatures: Record<string, SignatureBytes> = {};

  // Collect the order of signers from the first transaction
  for (const pubkey of Object.keys(firstSignedTx.signatures)) {
    signerOrder.push(pubkey);
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

  // Step 3: Build the result map preserving the order from the first transaction
  const result: Record<string, SignatureBytes> = {};
  for (const address of signerOrder) {
    const signature = allSignatures[address];
    if (!signature) {
      throw new Error(`Missing signature for: ${address}`);
    }
    result[address] = signature;
  }

  return result;
};

interface MultiSigBroadcastArgs {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  signedTxs: Awaited<ReturnType<typeof multisigOfflineSignWrap>>[];
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
}

// Combines, validates, and broadcasts outputs of multisigOfflineSignWrap()
export const multisigBroadcastWrap = async ({
  rpc,
  rpcSubscriptions,
  signedTxs,
  blockhash,
}: MultiSigBroadcastArgs) => {
  const messagesEqual = messageBytesEqual(signedTxs);
  if (!messagesEqual) throw new Error('Messages are not all the same');
  if (!signedTxs[0]) throw new Error('No signed transactions provided');

  const tx = {
    messageBytes: signedTxs[0].messageBytes,
    signatures: combineSignatures(signedTxs),
    lifetimeConstraint: blockhash,
  };

  assertTransactionIsFullySigned(tx);

  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(tx, { commitment: 'confirmed' });

  return tx;
};

interface SingleSignerWrapArgs {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner | Address; // Fee payer and default transfer authority
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  transferAuthority?: Address | TransactionSigner; // Defaults to payer if not provided
  unwrappedMint?: Address; // Will fetch from unwrappedTokenAccount if not provided
  recipientWrappedTokenAccount?: Address; // Defaults to payer's ATA if not provided
  unwrappedTokenProgram?: Address; // Will fetch from unwrappedTokenAccount owner if not provided
}

export const executeSingleSignerWrap = async ({
  rpc,
  rpcSubscriptions,
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
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

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

  const signedTx = await buildWrapTransaction({
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

  assertTransactionIsFullySigned(signedTx);

  const signature = getSignatureFromTransaction(signedTx);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTx, { commitment: 'confirmed' });

  return {
    recipientWrappedTokenAccount,
    escrowAccount,
    amount: BigInt(amount),
    signature,
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
  rpc: Rpc<SolanaRpcApi>;
  payer: KeyPairSigner | Address;
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
        owner: 'address' in payer ? payer.address : payer,
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
    tx =>
      typeof payer === 'string'
        ? setTransactionMessageFeePayer(payer, tx)
        : setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([wrapInstruction], tx),
    tx => partiallySignTransactionMessageWithSigners(tx),
  );
};
