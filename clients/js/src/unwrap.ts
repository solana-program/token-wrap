import {
  Address,
  appendTransactionMessageInstructions,
  CompilableTransactionMessage,
  createTransactionMessage,
  fetchEncodedAccount,
  GetAccountInfoApi,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  TransactionMessageWithBlockhashLifetime,
  TransactionSigner,
} from '@solana/kit';
import { getTokenDecoder } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, getUnwrapInstruction, UnwrapInput } from './generated';
import { Blockhash } from '@solana/rpc-types';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';

export interface SingleSignerUnwrapArgs {
  rpc: Rpc<GetAccountInfoApi>;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  payer: TransactionSigner; // Fee payer and default transfer authority
  wrappedTokenAccount: Address;
  unwrappedEscrow: Address;
  amount: bigint | number;
  recipientUnwrappedToken: Address;
  // Optional arguments below (will be derived/defaulted if not provided)
  transferAuthority?: Address | TransactionSigner; // Defaults to payer
  unwrappedMint?: Address; // Will derive from unwrappedEscrow if not provided
  wrappedTokenProgram?: Address; // Will derive from wrappedTokenAccount if not provided
  unwrappedTokenProgram?: Address; // Will derive from unwrappedEscrow if not provided
}

async function resolveUnwrapAddrs({
  rpc,
  payer,
  wrappedTokenAccount,
  unwrappedEscrow,
  inputUnwrappedMint,
  inputTransferAuthority,
  inputWrappedTokenProgram,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<GetAccountInfoApi>;
  payer: TransactionSigner;
  wrappedTokenAccount: Address;
  unwrappedEscrow: Address;
  inputUnwrappedMint?: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputWrappedTokenProgram?: Address;
  inputUnwrappedTokenProgram?: Address;
}) {
  const wrappedTokenProgram =
    inputWrappedTokenProgram ?? (await getOwnerFromAccount(rpc, wrappedTokenAccount));
  const unwrappedTokenProgram =
    inputUnwrappedTokenProgram ?? (await getOwnerFromAccount(rpc, unwrappedEscrow));

  // Get unwrapped mint from escrow account
  const unwrappedMint = inputUnwrappedMint ?? (await getMintFromTokenAccount(rpc, unwrappedEscrow));

  // Get wrapped mint from the token account being burned
  const wrappedAccountInfo = await fetchEncodedAccount(rpc, wrappedTokenAccount);
  if (!wrappedAccountInfo.exists) {
    throw new Error(`Wrapped token account ${wrappedTokenAccount} not found.`);
  }
  const wrappedMint = getTokenDecoder().decode(wrappedAccountInfo.data).mint;

  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });

  // Default transfer authority to payer if not provided
  const transferAuthority = inputTransferAuthority ?? payer;

  return {
    unwrappedMint,
    wrappedMint,
    wrappedMintAuthority,
    transferAuthority,
    wrappedTokenProgram,
    unwrappedTokenProgram,
  };
}

interface UnwrapTxBuilderArgs {
  payer: TransactionSigner;
  unwrappedEscrow: Address;
  wrappedTokenAccount: Address;
  amount: bigint | number;
  wrappedMint: Address;
  wrappedMintAuthority: Address;
  unwrappedMint: Address;
  recipientUnwrappedToken: Address;
  unwrappedTokenProgram: Address;
  wrappedTokenProgram: Address;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  transferAuthority: Address | TransactionSigner;
  multiSigners?: TransactionSigner[];
}

function buildUnwrapTransaction({
  payer,
  unwrappedEscrow,
  recipientUnwrappedToken,
  wrappedMintAuthority,
  unwrappedMint,
  wrappedTokenProgram,
  unwrappedTokenProgram,
  wrappedTokenAccount,
  wrappedMint,
  transferAuthority,
  amount,
  blockhash,
  multiSigners = [],
}: UnwrapTxBuilderArgs): CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime {
  const unwrapInstructionInput: UnwrapInput = {
    unwrappedEscrow,
    recipientUnwrappedToken,
    wrappedMintAuthority,
    unwrappedMint,
    wrappedTokenProgram,
    unwrappedTokenProgram,
    wrappedTokenAccount,
    wrappedMint,
    transferAuthority,
    amount: BigInt(amount),
    multiSigners,
  };

  const unwrapInstruction = getUnwrapInstruction(unwrapInstructionInput);

  return pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([unwrapInstruction], tx),
  );
}

export interface SingleSignerUnwrapResult {
  tx: CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime;
  recipientUnwrappedToken: Address;
  amount: bigint;
}

/**
 * Creates, signs (single signer or default authority), and sends an unwrap transaction.
 * Derives necessary PDAs and default accounts if not provided.
 */
export async function singleSignerUnwrapTx({
  rpc,
  blockhash,
  payer,
  wrappedTokenAccount,
  unwrappedEscrow,
  amount,
  recipientUnwrappedToken,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  wrappedTokenProgram: inputWrappedTokenProgram,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerUnwrapArgs): Promise<SingleSignerUnwrapResult> {
  const {
    wrappedMint,
    wrappedMintAuthority,
    transferAuthority,
    unwrappedTokenProgram,
    unwrappedMint,
    wrappedTokenProgram,
  } = await resolveUnwrapAddrs({
    rpc,
    payer,
    wrappedTokenAccount,
    unwrappedEscrow,
    inputUnwrappedMint,
    inputTransferAuthority,
    inputWrappedTokenProgram,
    inputUnwrappedTokenProgram,
  });

  const tx = buildUnwrapTransaction({
    payer,
    unwrappedEscrow,
    recipientUnwrappedToken,
    wrappedMintAuthority,
    unwrappedMint,
    wrappedTokenProgram,
    unwrappedTokenProgram,
    wrappedTokenAccount,
    wrappedMint,
    transferAuthority,
    amount,
    blockhash,
  });

  return {
    recipientUnwrappedToken,
    amount: BigInt(amount),
    tx,
  };
}

export interface MultiSignerUnWrapTxBuilderArgs extends UnwrapTxBuilderArgs {
  multiSigners: TransactionSigner[];
}

// Used to collect signatures
export function multisigOfflineSignUnwrap(
  args: MultiSignerUnWrapTxBuilderArgs,
): CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime {
  return buildUnwrapTransaction(args);
}
