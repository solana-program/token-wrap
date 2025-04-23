import { findWrappedMintAuthorityPda, findWrappedMintPda } from './generated';
import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  CompilableTransactionMessage,
  containsBytes,
  createTransactionMessage,
  fetchEncodedAccount,
  FullySignedTransaction,
  generateKeyPairSigner,
  GetAccountInfoApi,
  GetMinimumBalanceForRentExemptionApi,
  KeyPairSigner,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  SignatureBytes,
  Transaction,
  TransactionMessageWithBlockhashLifetime,
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

function getInitializeTokenFn(tokenProgram: Address) {
  if (tokenProgram === TOKEN_PROGRAM_ADDRESS) return initializeToken;
  if (tokenProgram === TOKEN_2022_PROGRAM_ADDRESS) return initializeToken2022;
  throw new Error(`${tokenProgram} is not a valid token program.`);
}

export async function createTokenAccountTx({
  rpc,
  blockhash,
  payer,
  mint,
  owner,
  tokenProgram,
}: {
  rpc: Rpc<GetMinimumBalanceForRentExemptionApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  payer: KeyPairSigner;
  mint: Address;
  owner: Address;
  tokenProgram: Address;
}) {
  const [keyPair, lamports] = await Promise.all([
    generateKeyPairSigner(),
    rpc.getMinimumBalanceForRentExemption(165n).send(),
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
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([createAccountIx, initializeAccountIx], tx),
  );

  return {
    tx,
    keyPair,
  };
}

export interface CreateEscrowAccountTxArgs {
  rpc: Rpc<GetAccountInfoApi & GetMinimumBalanceForRentExemptionApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  payer: KeyPairSigner;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
}

export interface CreateEscrowAccountTxResult {
  tx: CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime;
  keyPair: KeyPairSigner;
}

export async function createEscrowAccountTx({
  rpc,
  blockhash,
  payer,
  unwrappedMint,
  wrappedTokenProgram,
}: CreateEscrowAccountTxArgs): Promise<CreateEscrowAccountTxResult> {
  const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
  const unwrappedTokenProgram = await getOwnerFromAccount(rpc, unwrappedMint);

  return createTokenAccountTx({
    rpc,
    blockhash,
    payer,
    mint: unwrappedMint,
    owner: wrappedMintAuthority,
    tokenProgram: unwrappedTokenProgram,
  });
}

export async function getOwnerFromAccount(
  rpc: Rpc<GetAccountInfoApi>,
  accountAddress: Address,
): Promise<Address> {
  const accountInfo = await rpc.getAccountInfo(accountAddress, { encoding: 'base64' }).send();
  if (!accountInfo.value) {
    throw new Error(`Account ${accountAddress} not found.`);
  }
  return accountInfo.value.owner;
}

export async function getMintFromTokenAccount(
  rpc: Rpc<GetAccountInfoApi>,
  tokenAccountAddress: Address,
): Promise<Address> {
  const account = await fetchEncodedAccount(rpc, tokenAccountAddress);
  if (!account.exists) {
    throw new Error(`Unwrapped token account ${tokenAccountAddress} not found.`);
  }
  return getTokenDecoder().decode(account.data).mint;
}

function messageBytesEqual(results: (Transaction & TransactionWithBlockhashLifetime)[]): boolean {
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
}

function combineSignatures(
  signedTxs: (Transaction & TransactionWithBlockhashLifetime)[],
): Record<string, SignatureBytes> {
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

  return allSignatures as Record<string, SignatureBytes>;
}

export interface MultiSigCombineArgs {
  signedTxs: (Transaction & TransactionWithBlockhashLifetime)[];
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
}

// Combines, validates, and broadcasts outputs of multisig offline partially signed txs
export function combinedMultisigTx({
  signedTxs,
  blockhash,
}: MultiSigCombineArgs): FullySignedTransaction & TransactionWithBlockhashLifetime {
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
}
