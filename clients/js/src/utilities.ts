import { findWrappedMintAuthorityPda, findWrappedMintPda } from './generated';
import {
  Address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  generateKeyPairSigner,
  KeyPairSigner,
  pipe,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  SolanaRpcApi,
  SolanaRpcSubscriptionsApi,
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
