import {
  address,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import { findWrappedMintPda } from '../generated';
import { executeSingleSignerWrap } from '../wrap';

import { createEscrowAccount, createTokenAccount } from '../utilities';
import { executeCreateMint } from '../create-mint';
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
  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);

  // Initialize the wrapped mint
  const createMintResult = await executeCreateMint({
    rpc,
    rpcSubscriptions,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  console.log('======== Create Mint Successful ========');
  console.log('Wrapped Mint:', createMintResult.wrappedMint);
  console.log('Backpointer:', createMintResult.backpointer);
  console.log('Funded wrapped mint lamports:', createMintResult.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', createMintResult.fundedBackpointerLamports);
  console.log('Signature:', createMintResult.signature);

  // Setup accounts needed for wrap
  const escrowAccount = await createEscrowAccount({
    rpc,
    rpcSubscriptions,
    payer,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });

  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const recipientWrappedTokenAccount = await createTokenAccount({
    rpc,
    rpcSubscriptions,
    payer,
    mint: wrappedMint,
    tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    owner: payer.address,
  });

  const wrapResult = await executeSingleSignerWrap({
    rpc,
    rpcSubscriptions,
    payer,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount,
  });

  console.log('======== Wrap Successful ========');
  console.log('Wrap amount:', wrapResult.amount);
  console.log('Recipient account:', wrapResult.recipientWrappedTokenAccount);
  console.log('Escrow Account:', wrapResult.escrowAccount);
  console.log('Signature:', wrapResult.signature);
};

void main();
