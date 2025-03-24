import {
  address,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
} from '@solana/kit';
import { executeCreateMint } from './index';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';

// Replace these consts with your own
const PRIVATE_KEY_PAIR = new Uint8Array([
  58, 188, 194, 176, 230, 94, 253, 2, 24, 163, 198, 177, 92, 79, 213, 87, 122, 150, 216, 175, 176,
  159, 113, 144, 148, 82, 149, 249, 242, 255, 7, 1, 73, 203, 66, 98, 4, 2, 141, 236, 49, 10, 47,
  188, 93, 170, 111, 125, 44, 155, 4, 124, 48, 18, 188, 30, 158, 78, 158, 34, 44, 100, 61, 21,
]);
const UNWRAPPED_MINT_ADDRESS = address('5HXwCPsqa8cZSAXDimAW9vJB8b3VdjCMWt1aLrCT2Wpb');

const main = async () => {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);

  const result = await executeCreateMint({
    rpc,
    rpcSubscriptions,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  console.log('Wrapped Mint:', result.wrappedMint);
  console.log('Backpointer:', result.backpointer);
  console.log('Funded wrapped mint lamports:', result.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', result.fundedBackpointerLamports);
  console.log('Signature:', result.signature);
};

void main();
