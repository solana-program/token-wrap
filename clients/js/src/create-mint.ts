import {
  Address,
  appendTransactionMessageInstruction,
  compileTransaction,
  createSolanaRpc,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  KeyPairSigner,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
} from '@solana/kit';
import { getMintSize } from '@solana-program/token-2022';
import { IInstruction } from '@solana/instructions';
import { getTransferSolInstruction } from '@solana-program/system';
import {
  findBackpointerPda,
  findWrappedMintPda,
  getBackpointerSize,
  getCreateMintInstruction,
  TOKEN_WRAP_PROGRAM_ADDRESS,
} from './generated';

export const executeCreateMint = async ({
  rpc,
  unwrappedMint,
  wrappedTokenProgram,
  payer,
  idempotent = false,
}: {
  rpc: ReturnType<typeof createSolanaRpc>;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
  payer: KeyPairSigner;
  idempotent: boolean;
}) => {
  const [wrappedMint] = await findWrappedMintPda(
    {
      unwrappedMint,
      wrappedTokenProgram: wrappedTokenProgram,
    },
    { programAddress: TOKEN_WRAP_PROGRAM_ADDRESS },
  );
  const [backpointer] = await findBackpointerPda(
    { wrappedMint },
    { programAddress: TOKEN_WRAP_PROGRAM_ADDRESS },
  );

  const instructions: IInstruction[] = [];

  // Fund wrapped mint account if needed
  let fundedWrappedMintLamports = 0n;
  const wrappedMintAccount = await rpc.getAccountInfo(wrappedMint).send();
  const mintSize = BigInt(getMintSize());
  const wrappedMintRent = await rpc.getMinimumBalanceForRentExemption(mintSize).send();
  const wrappedMintLamports = wrappedMintAccount.value?.lamports ?? 0n;
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
  const backpointerAccount = await rpc.getAccountInfo(backpointer).send();
  const backpointerSize = BigInt(getBackpointerSize());
  const backpointerRent = await rpc.getMinimumBalanceForRentExemption(backpointerSize).send();
  const backpointerLamports = backpointerAccount.value?.lamports ?? 0n;
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
  let tx = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayer(payer.address, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
  );

  for (const instruction of instructions) {
    tx = appendTransactionMessageInstruction(instruction, tx);
  }

  const compiledTransaction = compileTransaction(tx);

  // Sign and send
  const [signatures] = await payer.signTransactions([compiledTransaction]);
  if (!signatures) {
    throw new Error('Expected a signature for compiled transaction');
  }

  const wireFormatTransaction = getBase64EncodedWireTransaction({
    ...compiledTransaction,
    signatures,
  });

  const signature = await rpc.sendTransaction(wireFormatTransaction, { encoding: 'base64' }).send();

  return {
    wrappedMint,
    backpointer,
    signature,
    fundedWrappedMintLamports,
    fundedBackpointerLamports,
  };
};
