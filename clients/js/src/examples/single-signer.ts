import {
  address,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getSignatureFromTransaction,
  sendAndConfirmTransactionFactory,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import {
  createEscrowAccountTx,
  findWrappedMintPda,
  createMintTx,
  singleSignerUnwrapTx,
  singleSignerWrapTx,
} from '../index';
import { createTokenAccountTx } from '../utilities';

// Replace these consts with your own
const PRIVATE_KEY_PAIR = new Uint8Array([
  242, 30, 38, 177, 152, 71, 235, 193, 93, 30, 119, 131, 42, 186, 202, 7, 45, 250, 126, 135, 107,
  137, 38, 91, 202, 212, 12, 8, 154, 213, 163, 200, 23, 237, 17, 163, 3, 135, 34, 126, 235, 146,
  251, 18, 199, 101, 153, 249, 134, 88, 219, 68, 167, 136, 234, 195, 12, 34, 184, 85, 234, 25, 125,
  94,
]);
const UNWRAPPED_MINT_ADDRESS = address('FAbYm8kdDsyc6csvTXPMBwCJDjTVkZcvrnyVVTSF74hU');
const UNWRAPPED_TOKEN_ACCOUNT = address('4dSPDdFuTbKTuJDDtTd8SUdbH6QY42hpTPRi6RRzzsPF');
const AMOUNT_TO_WRAP = 100n;

async function main() {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  // Initialize the wrapped mint
  const createMintMessage = await createMintTx({
    rpc,
    blockhash,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  const signedCreateMintTx = await signTransactionMessageWithSigners(createMintMessage.tx);
  await sendAndConfirm(signedCreateMintTx, { commitment: 'confirmed' });
  const createMintSignature = getSignatureFromTransaction(signedCreateMintTx);

  console.log('======== Create Mint Successful ========');
  console.log('Wrapped Mint:', createMintMessage.wrappedMint);
  console.log('Backpointer:', createMintMessage.backpointer);
  console.log('Funded wrapped mint lamports:', createMintMessage.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', createMintMessage.fundedBackpointerLamports);
  console.log('Signature:', createMintSignature);

  // === Setup accounts needed for wrap ===

  // Create escrow account that with hold unwrapped tokens
  const createEscrowMessage = await createEscrowAccountTx({
    rpc,
    blockhash,
    payer,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const signedCreateEscrowTx = await signTransactionMessageWithSigners(createEscrowMessage.tx);
  await sendAndConfirm(signedCreateEscrowTx, { commitment: 'confirmed' });

  // Create recipient account where wrapped tokens will be minted to
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const recipientTokenAccountMessage = await createTokenAccountTx({
    rpc,
    blockhash,
    payer,
    mint: wrappedMint,
    tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    owner: payer.address,
  });
  const signedRecipientAccountTx = await signTransactionMessageWithSigners(
    recipientTokenAccountMessage.tx,
  );
  await sendAndConfirm(signedRecipientAccountTx, { commitment: 'confirmed' });

  // Execute wrap
  const wrapMessage = await singleSignerWrapTx({
    rpc,
    blockhash,
    payer,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount: createEscrowMessage.keyPair.address,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount: recipientTokenAccountMessage.keyPair.address,
  });

  const signedWrapTx = await signTransactionMessageWithSigners(wrapMessage.tx);
  await sendAndConfirm(signedWrapTx, { commitment: 'confirmed' });
  const wrapSignature = getSignatureFromTransaction(signedWrapTx);

  console.log('======== Wrap Successful ========');
  console.log('Wrap amount:', wrapMessage.amount);
  console.log('Recipient account:', wrapMessage.recipientWrappedTokenAccount);
  console.log('Escrow Account:', wrapMessage.escrowAccount);
  console.log('Signature:', wrapSignature);

  const unwrapMessage = await singleSignerUnwrapTx({
    rpc,
    blockhash,
    payer,
    wrappedTokenAccount: recipientTokenAccountMessage.keyPair.address,
    unwrappedEscrow: createEscrowMessage.keyPair.address,
    amount: AMOUNT_TO_WRAP,
    recipientUnwrappedToken: UNWRAPPED_TOKEN_ACCOUNT,
  });

  const signedUnwrapTx = await signTransactionMessageWithSigners(unwrapMessage.tx);
  await sendAndConfirm(signedUnwrapTx, { commitment: 'confirmed' });
  const unwrapSignature = getSignatureFromTransaction(signedUnwrapTx);

  console.log('======== Unwrap Successful ========');
  console.log('Unwrapped amount:', unwrapMessage.amount);
  console.log('Recipient account:', unwrapMessage.recipientUnwrappedToken);
  console.log('Signature:', unwrapSignature);
}

void main();
