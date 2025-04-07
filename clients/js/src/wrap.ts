import {
  Address,
  appendTransactionMessageInstructions,
  assertTransactionIsFullySigned,
  createTransactionMessage,
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
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getWrapInstruction,
  WrapInput,
} from './generated';
import { Blockhash } from '@solana/rpc-types';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';

interface TxBuilderArgs {
  payer: KeyPairSigner | Address;
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

// Used to collect signatures
export const multisigOfflineSignWrap = async (args: Required<TxBuilderArgs>) => {
  return buildWrapTransaction(args);
};

interface SingleSignerWrapArgs {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner | Address; // Fee payer and default transfer authority
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  transferAuthority?: Address | TransactionSigner; // Defaults to payer if not provided
  unwrappedMint?: Address; // Will fetch from unwrappedTokenAccount if not provided
  recipientWrappedTokenAccount?: Address; // Defaults to payer's ATA if not provided
  unwrappedTokenProgram?: Address; // Will fetch from unwrappedTokenAccount owner if not provided
}

export const executeSingleSignerWrap = async ({
  rpc,
  rpcSubscriptions,
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  recipientWrappedTokenAccount: inputRecipientTokenAccount,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerWrapArgs) => {
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

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

  const signedTx = await buildWrapTransaction({
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

  assertTransactionIsFullySigned(signedTx);

  const signature = getSignatureFromTransaction(signedTx);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTx, { commitment: 'confirmed' });

  return {
    recipientWrappedTokenAccount,
    escrowAccount,
    amount: BigInt(amount),
    signature,
  };
};

// Meant to handle all of the potential default values
const resolveAddrs = async ({
  rpc,
  payer,
  unwrappedTokenAccount,
  wrappedTokenProgram,
  inputTransferAuthority,
  inputUnwrappedMint,
  inputRecipientTokenAccount,
  inputUnwrappedTokenProgram,
}: {
  rpc: Rpc<SolanaRpcApi>;
  payer: KeyPairSigner | Address;
  unwrappedTokenAccount: Address;
  wrappedTokenProgram: Address;
  inputTransferAuthority?: Address | TransactionSigner;
  inputUnwrappedMint?: Address;
  inputRecipientTokenAccount?: Address;
  inputUnwrappedTokenProgram?: Address;
}) => {
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
        owner: 'address' in payer ? payer.address : payer,
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
};

const buildWrapTransaction = async ({
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
}: TxBuilderArgs) => {
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
    tx =>
      typeof payer === 'string'
        ? setTransactionMessageFeePayer(payer, tx)
        : setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([wrapInstruction], tx),
    tx => partiallySignTransactionMessageWithSigners(tx),
  );
};
