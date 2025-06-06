import { findWrappedMintAuthorityPda, findWrappedMintPda } from './generated';
import {
  Address,
  assertTransactionIsFullySigned,
  containsBytes,
  fetchEncodedAccount,
  FullySignedTransaction,
  generateKeyPairSigner,
  GetAccountInfoApi,
  GetMinimumBalanceForRentExemptionApi,
  IInstruction,
  KeyPairSigner,
  Rpc,
  SignatureBytes,
  Transaction,
  TransactionWithBlockhashLifetime,
} from '@solana/kit';
import { getCreateAccountInstruction } from '@solana-program/system';
import {
  getInitializeAccountInstruction as initializeToken,
  TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';
import {
  fetchMaybeToken,
  findAssociatedTokenPda,
  getCreateAssociatedTokenInstruction,
  getInitializeAccountInstruction as initializeToken2022,
  getTokenDecoder,
  Token,
  TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';
import { Blockhash } from '@solana/rpc-types';
import { Account } from '@solana/accounts';

function getInitializeTokenFn(tokenProgram: Address) {
  if (tokenProgram === TOKEN_PROGRAM_ADDRESS) return initializeToken;
  if (tokenProgram === TOKEN_2022_PROGRAM_ADDRESS) return initializeToken2022;
  throw new Error(`${tokenProgram} is not a valid token program.`);
}

export async function createTokenAccount({
  rpc,
  payer,
  mint,
  owner,
  tokenProgram,
}: {
  rpc: Rpc<GetMinimumBalanceForRentExemptionApi>;
  payer: KeyPairSigner;
  mint: Address;
  owner: Address;
  tokenProgram: Address;
}): Promise<{ ixs: IInstruction[]; keyPair: KeyPairSigner }> {
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

  return {
    ixs: [createAccountIx, initializeAccountIx],
    keyPair,
  };
}

export interface CreateEscrowAccountArgs {
  rpc: Rpc<GetAccountInfoApi & GetMinimumBalanceForRentExemptionApi>;
  payer: KeyPairSigner;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
}

export type CreateEscrowAccountResult =
  | { kind: 'already_exists'; account: Account<Token> }
  | {
      kind: 'instructions_to_create';
      address: Address;
      ixs: IInstruction[];
    };

export async function createEscrowAccount({
  rpc,
  payer,
  unwrappedMint,
  wrappedTokenProgram,
}: CreateEscrowAccountArgs): Promise<CreateEscrowAccountResult> {
  const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
  const unwrappedTokenProgram = await getOwnerFromAccount(rpc, unwrappedMint);

  const [escrowAta] = await findAssociatedTokenPda({
    owner: wrappedMintAuthority,
    mint: unwrappedMint,
    tokenProgram: unwrappedTokenProgram,
  });

  const escrowResult = await fetchMaybeToken(rpc, escrowAta);
  if (escrowResult.exists) {
    return { kind: 'already_exists', account: escrowResult };
  }

  const ix = getCreateAssociatedTokenInstruction({
    payer,
    owner: wrappedMintAuthority,
    mint: unwrappedMint,
    ata: escrowAta,
    tokenProgram: unwrappedTokenProgram,
  });

  return { address: escrowAta, ixs: [ix], kind: 'instructions_to_create' };
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
  return results.every(
    c =>
      reference.messageBytes.length === c.messageBytes.length &&
      containsBytes(reference.messageBytes, c.messageBytes, 0),
  );
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
