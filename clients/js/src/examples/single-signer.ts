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
import { findWrappedMintPda } from '../generated';
import { singleSignerWrapTx } from '../wrap';

import { createEscrowAccountTx, createTokenAccountTx } from '../utilities';
import { createMintTx } from '../create-mint';
//
// Replace these consts with your own
const PRIVATE_KEY_PAIR = new Uint8Array([
  58, 188, 194, 176, 230, 94, 253, 2, 24, 163, 198, 177, 92, 79, 213, 87, 122, 150, 216, 175, 176,
  159, 113, 144, 148, 82, 149, 249, 242, 255, 7, 1, 73, 203, 66, 98, 4, 2, 141, 236, 49, 10, 47,
  188, 93, 170, 111, 125, 44, 155, 4, 124, 48, 18, 188, 30, 158, 78, 158, 34, 44, 100, 61, 21,
]);
const UNWRAPPED_MINT_ADDRESS = address('5StBUZ2w8ShDN9iF7NkGpDNNH2wv9jK7zhArmVRpwrCt');
const UNWRAPPED_TOKEN_ACCOUNT = address('CbuRmvG3frMoPFnsKfC2t8jTUHFjtnrKZBt2aqdqH4PG');
const AMOUNT_TO_WRAP = 100n;

const main = async () => {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);

  // Initialize the wrapped mint
  const createMintMessage = await createMintTx({
    rpc,
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
  const signature = getSignatureFromTransaction(wrapMessage.tx);

  console.log('======== Wrap Successful ========');
  console.log('Wrap amount:', wrapMessage.amount);
  console.log('Recipient account:', wrapMessage.recipientWrappedTokenAccount);
  console.log('Escrow Account:', wrapMessage.escrowAccount);
  console.log('Signature:', signature);
};

void main();
