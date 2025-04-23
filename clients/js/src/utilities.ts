import { findWrappedMintAuthorityPda, findWrappedMintPda } from './generated';
import {
  Address,
  appendTransactionMessageInstructions,
  CompilableTransactionMessage,
  createTransactionMessage,
  generateKeyPairSigner,
  GetAccountInfoApi,
  GetMinimumBalanceForRentExemptionApi,
  KeyPairSigner,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  TransactionMessageWithBlockhashLifetime,
} from '@solana/kit';
import { getCreateAccountInstruction } from '@solana-program/system';
import {
  getInitializeAccountInstruction as initializeToken,
  TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';
import {
  getInitializeAccountInstruction as initializeToken2022,
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
