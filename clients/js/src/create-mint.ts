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
import {
  BACKPOINTER_SIZE,
  getBackpointerAddress,
  getWrappedMintAddress,
  MINT_SIZE,
  TOKEN_WRAP_PROGRAM_ID,
} from './common';
import { AccountRole, IInstruction } from '@solana/instructions';
import { getTransferSolInstruction, SYSTEM_PROGRAM_ADDRESS } from '@solana-program/system';

export enum TokenWrapInstruction {
  CreateMint = 0,
  Wrap = 1,
  Unwrap = 2,
}

const getCreateWrappedMintInstruction = ({
  wrappedMint,
  backpointer,
  unwrappedMint,
  wrappedTokenProgramId,
  idempotent = false,
}: {
  wrappedMint: Address;
  backpointer: Address;
  unwrappedMint: Address;
  wrappedTokenProgramId: Address;
  idempotent: boolean;
}): IInstruction => {
  const data = new Uint8Array([TokenWrapInstruction.CreateMint, idempotent ? 1 : 0]);

  return {
    programAddress: TOKEN_WRAP_PROGRAM_ID,
    accounts: [
      { address: wrappedMint, role: AccountRole.WRITABLE },
      { address: backpointer, role: AccountRole.WRITABLE },
      { address: unwrappedMint, role: AccountRole.READONLY },
      { address: SYSTEM_PROGRAM_ADDRESS, role: AccountRole.READONLY },
      { address: wrappedTokenProgramId, role: AccountRole.READONLY },
    ],
    data,
  };
};

export const createMint = async ({
  rpc,
  unwrappedMint,
  wrappedTokenProgramId,
  payer,
  idempotent = false,
}: {
  rpc: ReturnType<typeof createSolanaRpc>;
  unwrappedMint: Address;
  wrappedTokenProgramId: Address;
  payer: KeyPairSigner;
  idempotent: boolean;
}) => {
  const wrappedMint = await getWrappedMintAddress(unwrappedMint, wrappedTokenProgramId);
  const backpointer = await getBackpointerAddress(wrappedMint);

  const instructions: IInstruction[] = [];

  // Fund wrapped mint account if needed
  let fundedWrappedMintLamports = 0n;
  const wrappedMintAccount = await rpc.getAccountInfo(wrappedMint).send();
  const wrappedMintRent = await rpc.getMinimumBalanceForRentExemption(MINT_SIZE).send();
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
  const backpointerRent = await rpc.getMinimumBalanceForRentExemption(BACKPOINTER_SIZE).send();
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
    getCreateWrappedMintInstruction({
      wrappedMint,
      backpointer,
      unwrappedMint,
      wrappedTokenProgramId,
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
