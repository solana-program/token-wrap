import {
  Address,
  appendTransactionMessageInstructions,
  CompilableTransactionMessage,
  createTransactionMessage,
  GetAccountInfoApi,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  TransactionMessageWithBlockhashLifetime,
  TransactionSigner,
} from '@solana/kit';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getWrapInstruction,
  WrapInput,
} from './generated';
import { Blockhash } from '@solana/rpc-types';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';
import { findAssociatedTokenPda } from '@solana-program/token-2022';

interface TxBuilderArgs {
  payer: TransactionSigner;
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  wrappedMint: Address;
  wrappedMintAuthority: Address;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  transferAuthority: Address | TransactionSigner;
  unwrappedMint: Address;
  recipientWrappedTokenAccount: Address;
  unwrappedTokenProgram: Address;
  multiSigners?: TransactionSigner[];
}

export interface MultiSignerWrapTxBuilderArgs extends TxBuilderArgs {
  multiSigners: TransactionSigner[];
}

// Used to collect signatures
export function multisigOfflineSignWrapTx(
  args: MultiSignerWrapTxBuilderArgs,
): CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime {
  return buildWrapTransaction(args);
}

export interface SingleSignerWrapArgs {
  rpc: Rpc<GetAccountInfoApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  payer: TransactionSigner; // Fee payer and default transfer authority
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  transferAuthority?: Address | TransactionSigner; // Defaults to payer if not provided
  unwrappedMint?: Address; // Will fetch from unwrappedTokenAccount if not provided
  recipientWrappedTokenAccount?: Address; // Defaults to payer's ATA if not provided
  unwrappedTokenProgram?: Address; // Will fetch from unwrappedTokenAccount owner if not provided
}

export interface SingleSignerWrapResult {
  tx: CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime;
  recipientWrappedTokenAccount: Address;
  escrowAccount: Address;
  amount: bigint;
}

export async function singleSignerWrapTx({
  rpc,
  blockhash,
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  recipientWrappedTokenAccount: inputRecipientTokenAccount,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerWrapArgs): Promise<SingleSignerWrapResult> {
  const {
    unwrappedMint,
    unwrappedTokenProgram,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
    transferAuthority,
  } = await resolveAddrs({
    rpc,
    payer,
    inputTransferAuthority,
    inputUnwrappedMint,
    unwrappedTokenAccount,
    inputUnwrappedTokenProgram,
    wrappedTokenProgram,
    inputRecipientTokenAccount,
  });

  const tx = buildWrapTransaction({
    blockhash,
    payer,
    unwrappedTokenAccount,
    escrowAccount,
    wrappedTokenProgram,
    amount,
    transferAuthority,
    unwrappedMint,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
    unwrappedTokenProgram,
  });

  return {
    tx,
    recipientWrappedTokenAccount,
    escrowAccount,
    amount: BigInt(amount),
  };
}

// Meant to handle all of the potential default values
async function resolveAddrs({
  rpc,
  payer,
  unwrappedTokenAccount,
  wrappedTokenProgram,
  inputTransferAuthority,
  inputUnwrappedMint,
  inputRecipientTokenAccount,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<GetAccountInfoApi>;
  payer: TransactionSigner;
  unwrappedTokenAccount: Address;
  wrappedTokenProgram: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputUnwrappedMint?: Address;
  inputRecipientTokenAccount?: Address;
  inputUnwrappedTokenProgram?: Address;
}) {
  const unwrappedMint =
    inputUnwrappedMint ?? (await getMintFromTokenAccount(rpc, unwrappedTokenAccount));
  const unwrappedTokenProgram =
    inputUnwrappedTokenProgram ?? (await getOwnerFromAccount(rpc, unwrappedTokenAccount));
  const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
  const recipientWrappedTokenAccount =
    inputRecipientTokenAccount ??
    (
      await findAssociatedTokenPda({
        owner: payer.address,
        mint: wrappedMint,
        tokenProgram: wrappedTokenProgram,
      })
    )[0];

  const transferAuthority = inputTransferAuthority ?? payer;

  return {
    transferAuthority,
    unwrappedMint,
    unwrappedTokenProgram,
    wrappedMint,
    wrappedMintAuthority,
    recipientWrappedTokenAccount,
  };
}

function buildWrapTransaction({
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority,
  unwrappedMint,
  recipientWrappedTokenAccount,
  unwrappedTokenProgram,
  wrappedMint,
  wrappedMintAuthority,
  blockhash,
  multiSigners = [],
}: TxBuilderArgs): CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime {
  const wrapInstructionInput: WrapInput = {
    recipientWrappedTokenAccount,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    wrappedTokenProgram,
    unwrappedTokenAccount,
    unwrappedMint,
    unwrappedEscrow: escrowAccount,
    transferAuthority,
    amount: BigInt(amount),
    multiSigners,
  };

  const wrapInstruction = getWrapInstruction(wrapInstructionInput);

  return pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([wrapInstruction], tx),
  );
}
