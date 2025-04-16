import {
  Address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  fetchEncodedAccount,
  GetMinimumBalanceForRentExemptionApi,
  IInstruction,
  KeyPairSigner,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  GetAccountInfoApi,
} from '@solana/kit';
import { getMintSize } from '@solana-program/token-2022';
import { getTransferSolInstruction } from '@solana-program/system';
import {
  findBackpointerPda,
  findWrappedMintPda,
  getBackpointerSize,
  getCreateMintInstruction,
} from './generated';
import { Blockhash } from '@solana/rpc-types';

export const createMintTx = async ({
  rpc,
  blockhash,
  unwrappedMint,
  wrappedTokenProgram,
  payer,
  idempotent = false,
}: {
  rpc: Rpc<GetAccountInfoApi & GetMinimumBalanceForRentExemptionApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
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

  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions(instructions, tx),
  );

  return {
    wrappedMint,
    backpointer,
    tx,
    fundedWrappedMintLamports,
    fundedBackpointerLamports,
  };
};
