import {
  Address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  fetchEncodedAccount,
  getSignatureFromTransaction,
  IInstruction,
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
import { getMintSize } from '@solana-program/token-2022';
import { getTransferSolInstruction } from '@solana-program/system';
import {
  findBackpointerPda,
  findWrappedMintPda,
  getBackpointerSize,
  getCreateMintInstruction,
} from './generated';

export const executeCreateMint = async ({
  rpc,
  rpcSubscriptions,
  unwrappedMint,
  wrappedTokenProgram,
  payer,
  idempotent = false,
}: {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
  payer: KeyPairSigner;
  idempotent: boolean;
}) => {
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint,
    wrappedTokenProgram: wrappedTokenProgram,
  });
  const [backpointer] = await findBackpointerPda({ wrappedMint });

  const instructions: IInstruction[] = [];

  // Fund wrapped mint account if needed
  let fundedWrappedMintLamports = 0n;

  const mintSize = BigInt(getMintSize());
  const [wrappedMintAccount, wrappedMintRent] = await Promise.all([
    fetchEncodedAccount(rpc, wrappedMint),
    rpc.getMinimumBalanceForRentExemption(mintSize).send(),
  ]);

  const wrappedMintLamports = wrappedMintAccount.exists ? wrappedMintAccount.lamports : 0n;
  if (wrappedMintLamports < wrappedMintRent) {
    fundedWrappedMintLamports = wrappedMintRent - wrappedMintLamports;
    instructions.push(
      getTransferSolInstruction({
        source: payer,
        destination: wrappedMint,
        amount: fundedWrappedMintLamports,
      }),
    );
  }

  // Fund backpointer account if needed
  let fundedBackpointerLamports = 0n;

  const backpointerSize = BigInt(getBackpointerSize());
  const [backpointerAccount, backpointerRent] = await Promise.all([
    fetchEncodedAccount(rpc, backpointer),
    rpc.getMinimumBalanceForRentExemption(backpointerSize).send(),
  ]);

  const backpointerLamports = backpointerAccount.exists ? backpointerAccount.lamports : 0n;
  if (backpointerLamports < backpointerRent) {
    fundedBackpointerLamports = backpointerRent - backpointerLamports;
    instructions.push(
      getTransferSolInstruction({
        source: payer,
        destination: backpointer,
        amount: fundedBackpointerLamports,
      }),
    );
  }

  // Add create_mint instruction
  instructions.push(
    getCreateMintInstruction({
      wrappedMint,
      backpointer,
      unwrappedMint,
      wrappedTokenProgram,
      idempotent,
    }),
  );

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  // Build transaction
  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    tx => appendTransactionMessageInstructions(instructions, tx),
  );

  // Send tx
  const signedTransaction = await signTransactionMessageWithSigners(tx);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTransaction, { commitment: 'confirmed' });

  return {
    wrappedMint,
    backpointer,
    signature: getSignatureFromTransaction(signedTransaction),
    fundedWrappedMintLamports,
    fundedBackpointerLamports,
  };
};
