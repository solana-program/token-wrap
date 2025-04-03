import {
  Address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  fetchEncodedAccount,
  getSignatureFromTransaction,
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
  TransactionSigner,
} from '@solana/kit';
import { getTokenDecoder } from '@solana-program/token-2022';
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getWrapInstruction,
  WrapInput,
} from './generated';

const getMintFromTokenAccount = async (
  rpc: Rpc<SolanaRpcApi>,
  tokenAccountAddress: Address,
): Promise<Address> => {
  const account = await fetchEncodedAccount(rpc, tokenAccountAddress);
  if (!account.exists) {
    throw new Error(`Unwrapped token account ${tokenAccountAddress} not found.`);
  }
  return getTokenDecoder().decode(account.data).mint;
};

const getOwnerFromAccount = async (
  rpc: Rpc<SolanaRpcApi>,
  accountAddress: Address,
): Promise<Address> => {
  const accountInfo = await rpc.getAccountInfo(accountAddress, { encoding: 'base64' }).send();
  if (!accountInfo.value) {
    throw new Error(`Account ${accountAddress} not found.`);
  }
  return accountInfo.value.owner;
};

export interface ExecuteWrapArgs {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  payer: KeyPairSigner; // Fee payer and default transfer authority
  unwrappedTokenAccount: Address;
  escrowAccount: Address;
  wrappedTokenProgram: Address;
  amount: bigint | number;
  transferAuthority?: Address | TransactionSigner; // Defaults to payer if not provided
  unwrappedMint?: Address; // Will fetch from unwrappedTokenAccount if not provided
  recipientTokenAccount?: Address; // Defaults to payer's ATA if not provided
  unwrappedTokenProgram?: Address; // Will fetch from unwrappedTokenAccount owner if not provided
  multiSigners?: TransactionSigner[]; // For multisig transfer authority
}

export const executeWrap = async ({
  rpc,
  rpcSubscriptions,
  payer,
  unwrappedTokenAccount,
  escrowAccount,
  wrappedTokenProgram,
  amount,
  transferAuthority: inputTransferAuthority,
  unwrappedMint: inputUnwrappedMint,
  recipientTokenAccount: inputRecipientTokenAccount,
  unwrappedTokenProgram: inputUnwrappedTokenProgram,
  multiSigners = [],
}: ExecuteWrapArgs) => {
  // --- 1. Resolve Addresses ---
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

  // --- 2. Create the Instruction ---
  const transferAuthority = inputTransferAuthority ?? payer;
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

  // --- 3. Build & Sign Transaction ---
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    tx => appendTransactionMessageInstructions([wrapInstruction], tx),
  );
  const signedTransaction = await signTransactionMessageWithSigners(tx);

  // --- 4. Send and Confirm Transaction ---
  const signature = getSignatureFromTransaction(signedTransaction);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTransaction, { commitment: 'confirmed' });

  return {
    recipientWrappedTokenAccount,
    escrowAccount,
    amount: BigInt(amount),
    signature,
  };
};
