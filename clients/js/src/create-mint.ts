import {
  Address,
  fetchEncodedAccount,
  GetAccountInfoApi,
  GetMinimumBalanceForRentExemptionApi,
  Instruction,
  KeyPairSigner,
  Rpc,
} from '@solana/kit';
import { getMintSize, TOKEN_2022_PROGRAM_ADDRESS, extension } from '@solana-program/token-2022';
import { getTransferSolInstruction } from '@solana-program/system';
import {
  findBackpointerPda,
  findWrappedMintPda,
  getBackpointerSize,
  getCreateMintInstruction,
} from './generated';

export interface CreateMintArgs {
  rpc: Rpc<GetAccountInfoApi & GetMinimumBalanceForRentExemptionApi>;
  unwrappedMint: Address;
  wrappedTokenProgram: Address;
  payer: KeyPairSigner;
  idempotent: boolean;
}

export interface CreateMintResult {
  wrappedMint: Address;
  backpointer: Address;
  fundedWrappedMintLamports: bigint;
  fundedBackpointerLamports: bigint;
  ixs: Instruction[];
}

// The on-chain program adds these two extensions by default. We must account for
// their size here. The `getMintSize` function from the library expects extension
// data objects, but since the size of these extensions is fixed, we can pass
// dummy/default values.
const DEFAULT_EXTENSIONS = [
  extension('ConfidentialTransferMint', {
    autoApproveNewAccounts: true,
    authority: null,
    auditorElgamalPubkey: null,
  }),
  extension('MetadataPointer', {
    authority: null,
    metadataAddress: null,
  }),
];

export async function createMint({
  rpc,
  unwrappedMint,
  wrappedTokenProgram,
  payer,
  idempotent = false,
}: CreateMintArgs): Promise<CreateMintResult> {
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint,
    wrappedTokenProgram: wrappedTokenProgram,
  });
  const [backpointer] = await findBackpointerPda({ wrappedMint });

  const instructions: Instruction[] = [];

  // Fund wrapped mint account if needed
  let fundedWrappedMintLamports = 0n;

  let mintSize = BigInt(getMintSize());
  if (wrappedTokenProgram === TOKEN_2022_PROGRAM_ADDRESS) {
    mintSize = BigInt(getMintSize(DEFAULT_EXTENSIONS));
  }

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
      }) as Instruction,
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
      }) as Instruction,
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

  return {
    wrappedMint,
    backpointer,
    ixs: instructions,
    fundedWrappedMintLamports,
    fundedBackpointerLamports,
  };
}
