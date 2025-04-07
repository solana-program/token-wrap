import { findWrappedMintAuthorityPda, findWrappedMintPda } from './generated';
import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  createTransactionMessage,
  fetchEncodedAccount,
  generateKeyPairSigner,
  KeyPairSigner,
  pipe,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  SignatureBytes,
  signTransactionMessageWithSigners,
  SolanaRpcApi,
  SolanaRpcSubscriptionsApi,
  Transaction,
  TransactionWithBlockhashLifetime,
} from '@solana/kit';
import { getCreateAccountInstruction } from '@solana-program/system';
import {
  getInitializeAccountInstruction as initializeToken,
  TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';
import {
  getInitializeAccountInstruction as initializeToken2022,
  getTokenDecoder,
  TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';
import { Blockhash } from '@solana/rpc-types';

const getInitializeTokenFn = (tokenProgram: Address) => {
  if (tokenProgram === TOKEN_PROGRAM_ADDRESS) return initializeToken;
  if (tokenProgram === TOKEN_2022_PROGRAM_ADDRESS) return initializeToken2022;
  throw new Error(`${tokenProgram} is not a valid token program.`);
};

export const createTokenAccount = async ({
  rpc,
  rpcSubscriptions,
  payer,
  mint,
  owner,
  tokenProgram,
}: {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner;
  mint: Address;
  owner: Address;
  tokenProgram: Address;
}): Promise<Address> => {
  const [keyPair, lamports, { value: latestBlockhash }] = await Promise.all([
    generateKeyPairSigner(),
    rpc.getMinimumBalanceForRentExemption(165n).send(),
    rpc.getLatestBlockhash().send(),
  ]);

  const createAccountIx = getCreateAccountInstruction({
    payer,
    newAccount: keyPair,
    lamports,
    space: 165,
    programAddress: tokenProgram,
  });

  const initializeAccountIx = getInitializeTokenFn(tokenProgram)({
    account: keyPair.address,
    mint,
    owner,
  });

  // Build and send the transaction
  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    tx => appendTransactionMessageInstructions([createAccountIx, initializeAccountIx], tx),
  );

  const signedTx = await signTransactionMessageWithSigners(tx);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTx, { commitment: 'confirmed' });

  return keyPair.address;
};

export const createEscrowAccount = async ({
  rpc,
  rpcSubscriptions,
  payer,
  unwrappedMint,
  wrappedTokenProgram,
}: {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
}) => {
  const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
  return createTokenAccount({
    rpc,
    rpcSubscriptions,
    payer,
    mint: unwrappedMint,
    owner: wrappedMintAuthority,
    tokenProgram: TOKEN_PROGRAM_ADDRESS,
  });
};

export const getMintFromTokenAccount = async (
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

export const messageBytesEqual = (
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

export const combineSignatures = (
  signedTxs: (Transaction & TransactionWithBlockhashLifetime)[],
) => {
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
  signedTxs: (Transaction & TransactionWithBlockhashLifetime)[];
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
}

// Combines, validates, and broadcasts outputs of multisigOfflineSignWrap() & multisigOfflineSignUnwrap()
export const multisigBroadcast = async ({
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
