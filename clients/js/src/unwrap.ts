import {
  Address,
  appendTransactionMessageInstructions,
  CompilableTransactionMessage,
  createTransactionMessage,
  fetchEncodedAccount,
  GetAccountInfoApi,
  IInstruction,
  pipe,
  Rpc,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  TransactionMessageWithBlockhashLifetime,
  TransactionSigner,
} from '@solana/kit';
import { findAssociatedTokenPda, getTokenDecoder } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, getUnwrapInstruction, UnwrapInput } from './generated';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';
import { Blockhash } from '@solana/rpc-types';

export interface SingleSignerUnwrapArgs {
  rpc: Rpc<GetAccountInfoApi>;
  payer: TransactionSigner; // Fee payer and default transfer authority
  wrappedTokenAccount: Address;
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
  recipientUnwrappedToken,
  inputUnwrappedMint,
  inputTransferAuthority,
  inputWrappedTokenProgram,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<GetAccountInfoApi>;
  payer: TransactionSigner;
  wrappedTokenAccount: Address;
  recipientUnwrappedToken: Address;
  inputUnwrappedMint?: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputWrappedTokenProgram?: Address;
  inputUnwrappedTokenProgram?: Address;
}) {
  const wrappedTokenProgram =
    inputWrappedTokenProgram ?? (await getOwnerFromAccount(rpc, wrappedTokenAccount));
  const unwrappedTokenProgram =
    inputUnwrappedTokenProgram ?? (await getOwnerFromAccount(rpc, recipientUnwrappedToken));
  const unwrappedMint =
    inputUnwrappedMint ?? (await getMintFromTokenAccount(rpc, recipientUnwrappedToken));

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
  wrappedTokenAccount: Address;
  amount: bigint | number;
  wrappedMint: Address;
  wrappedMintAuthority: Address;
  unwrappedMint: Address;
  recipientUnwrappedToken: Address;
  unwrappedTokenProgram: Address;
  wrappedTokenProgram: Address;
  transferAuthority: Address | TransactionSigner;
  multiSigners?: TransactionSigner[];
}

async function buildUnwrapTransaction({
  recipientUnwrappedToken,
  wrappedMintAuthority,
  unwrappedMint,
  wrappedTokenProgram,
  unwrappedTokenProgram,
  wrappedTokenAccount,
  wrappedMint,
  transferAuthority,
  amount,
  multiSigners = [],
}: UnwrapTxBuilderArgs): Promise<IInstruction> {
  const [unwrappedEscrow] = await findAssociatedTokenPda({
    owner: wrappedMintAuthority,
    mint: unwrappedMint,
    tokenProgram: unwrappedTokenProgram,
  });

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

  return getUnwrapInstruction(unwrapInstructionInput);
}

export interface SingleSignerUnwrapResult {
  ixs: IInstruction[];
  recipientUnwrappedToken: Address;
  amount: bigint;
}

/**
 * Creates, signs (single signer or default authority), and sends an unwrap transaction.
 * Derives necessary PDAs and default accounts if not provided.
 */
export async function singleSignerUnwrap({
  rpc,
  payer,
  wrappedTokenAccount,
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
    recipientUnwrappedToken,
    inputUnwrappedMint,
    inputTransferAuthority,
    inputWrappedTokenProgram,
    inputUnwrappedTokenProgram,
  });

  const ix = await buildUnwrapTransaction({
    recipientUnwrappedToken,
    wrappedMintAuthority,
    unwrappedMint,
    wrappedTokenProgram,
    unwrappedTokenProgram,
    wrappedTokenAccount,
    wrappedMint,
    transferAuthority,
    amount,
  });

  return {
    recipientUnwrappedToken,
    amount: BigInt(amount),
    ixs: [ix],
  };
}

export interface MultiSignerUnWrapTxBuilderArgs extends UnwrapTxBuilderArgs {
  payer: TransactionSigner;
  blockhash: {
    blockhash: Blockhash;
    lastValidBlockHeight: bigint;
  };
  multiSigners: TransactionSigner[];
}

// Used to collect signatures
export async function multisigOfflineSignUnwrap(
  args: MultiSignerUnWrapTxBuilderArgs,
): Promise<CompilableTransactionMessage & TransactionMessageWithBlockhashLifetime> {
  const unwrapIx = await buildUnwrapTransaction(args);

  return pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(args.payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(args.blockhash, tx),
    tx => appendTransactionMessageInstructions([unwrapIx], tx),
  );
}
