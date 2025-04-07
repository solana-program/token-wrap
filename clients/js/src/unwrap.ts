// src/unwrap.ts

import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  createTransactionMessage,
  fetchEncodedAccount,
  getSignatureFromTransaction,
  KeyPairSigner,
  partiallySignTransactionMessageWithSigners,
  pipe,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayer,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  SolanaRpcApi,
  SolanaRpcSubscriptionsApi,
  TransactionSigner,
} from '@solana/kit';
import { getTokenDecoder } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, getUnwrapInstruction, UnwrapInput } from './generated';
import { Blockhash } from '@solana/rpc-types';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';

interface UnwrapTxBuilderArgs {
  payer: KeyPairSigner | Address;
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

interface SingleSignerUnwrapArgs {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner | Address;
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

const resolveUnwrapAddrs = async ({
  rpc,
  payer,
  wrappedTokenAccount,
  unwrappedEscrow,
  inputUnwrappedMint,
  inputTransferAuthority,
  inputWrappedTokenProgram,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<SolanaRpcApi>;
  payer: KeyPairSigner | Address;
  wrappedTokenAccount: Address;
  unwrappedEscrow: Address;
  inputUnwrappedMint?: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputWrappedTokenProgram?: Address;
  inputUnwrappedTokenProgram?: Address;
}) => {
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
};

const buildUnwrapTransaction = async ({
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
}: UnwrapTxBuilderArgs) => {
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
    tx =>
      typeof payer === 'string'
        ? setTransactionMessageFeePayer(payer, tx)
        : setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([unwrapInstruction], tx),
    tx => partiallySignTransactionMessageWithSigners(tx),
  );
};

/**
 * Creates, signs (single signer or default authority), and sends an unwrap transaction.
 * Derives necessary PDAs and default accounts if not provided.
 */
export const executeSingleSignerUnwrap = async ({
  rpc,
  rpcSubscriptions,
  payer,
  wrappedTokenAccount,
  unwrappedEscrow,
  amount,
  recipientUnwrappedToken,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  wrappedTokenProgram: inputWrappedTokenProgram,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerUnwrapArgs) => {
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  const {
    wrappedMint,
    wrappedMintAuthority,
    transferAuthority,
    wrappedTokenProgram,
    unwrappedTokenProgram,
    unwrappedMint,
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

  const signedTx = await buildUnwrapTransaction({
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

  assertTransactionIsFullySigned(signedTx);

  const signature = getSignatureFromTransaction(signedTx);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTx, { commitment: 'confirmed' });

  return {
    recipientUnwrappedToken,
    amount: BigInt(amount),
    signature,
  };
};

/**
 * Creates and partially signs an unwrap transaction for offline multisig workflows.
 * Requires *all* accounts and signers to be explicitly provided.
 */
export const multisigOfflineSignUnwrap = async (args: Required<UnwrapTxBuilderArgs>) => {
  return buildUnwrapTransaction(args);
};
