import {
  address,
  appendTransactionMessageInstructions,
  assertIsSendableTransaction,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getSignatureFromTransaction,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import {
  createEscrowAccount,
  findWrappedMintPda,
  createMint,
  singleSignerUnwrap,
  singleSignerWrap,
} from '../index';
import { createTokenAccount } from '../utilities';

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
  const createMintHelper = await createMint({
    rpc,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  const createMintTx = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions(createMintHelper.ixs, tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(createMintTx);
  await sendAndConfirm(createMintTx, { commitment: 'confirmed' });
  const createMintSignature = getSignatureFromTransaction(createMintTx);

  console.log('======== Create Mint Successful ========');
  console.log('Wrapped Mint:', createMintHelper.wrappedMint);
  console.log('Backpointer:', createMintHelper.backpointer);
  console.log('Funded wrapped mint lamports:', createMintHelper.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', createMintHelper.fundedBackpointerLamports);
  console.log('Signature:', createMintSignature);

  // === Setup accounts needed for wrap ===

  // Create escrow account that with hold unwrapped tokens
  const createEscrowHelper = await createEscrowAccount({
    rpc,
    payer,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  if (createEscrowHelper.kind === 'instructions_to_create') {
    const createEscrowTx = await pipe(
      createTransactionMessage({ version: 0 }),
      tx => setTransactionMessageFeePayerSigner(payer, tx),
      tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
      tx => appendTransactionMessageInstructions(createEscrowHelper.ixs, tx),
      tx => signTransactionMessageWithSigners(tx),
    );
    assertIsSendableTransaction(createEscrowTx);
    await sendAndConfirm(createEscrowTx, { commitment: 'confirmed' });
    const createEscrowSignature = getSignatureFromTransaction(createEscrowTx);

    console.log('======== Create Escrow Successful ========');
    console.log('Escrow address:', createEscrowHelper.address);
    console.log('Signature:', createEscrowSignature);
  } else {
    console.log('======== Escrow already exists, skipping creation ========');
  }

  // Create recipient account where wrapped tokens will be minted to
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const recipientTokenAccountHelper = await createTokenAccount({
    rpc,
    payer,
    mint: wrappedMint,
    tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    owner: payer.address,
  });
  const recipientTokenAccountTx = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions(recipientTokenAccountHelper.ixs, tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(recipientTokenAccountTx);
  await sendAndConfirm(recipientTokenAccountTx, { commitment: 'confirmed' });

  // Execute wrap
  const wrapHelper = await singleSignerWrap({
    rpc,
    payer,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount: recipientTokenAccountHelper.keyPair.address,
  });

  const wrapTx = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions(wrapHelper.ixs, tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(wrapTx);
  await sendAndConfirm(wrapTx, { commitment: 'confirmed' });
  const wrapSignature = getSignatureFromTransaction(wrapTx);

  console.log('======== Wrap Successful ========');
  console.log('Wrap amount:', wrapHelper.amount);
  console.log('Recipient account:', wrapHelper.recipientWrappedTokenAccount);
  console.log('Escrow Account:', wrapHelper.escrowAccount);
  console.log('Signature:', wrapSignature);

  // execute unwrap

  const unwrapHelper = await singleSignerUnwrap({
    rpc,
    payer,
    wrappedTokenAccount: recipientTokenAccountHelper.keyPair.address,
    amount: AMOUNT_TO_WRAP,
    recipientUnwrappedToken: UNWRAPPED_TOKEN_ACCOUNT,
  });

  const unwrapTx = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions(unwrapHelper.ixs, tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(unwrapTx);
  await sendAndConfirm(unwrapTx, { commitment: 'confirmed' });
  const unwrapSignature = getSignatureFromTransaction(unwrapTx);

  console.log('======== Unwrap Successful ========');
  console.log('Unwrapped amount:', unwrapHelper.amount);
  console.log('Recipient account:', unwrapHelper.recipientUnwrappedToken);
  console.log('Signature:', unwrapSignature);
}

void main();
