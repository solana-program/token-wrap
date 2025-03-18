import { Address, address, getAddressEncoder } from '@solana/kit';
import { getProgramDerivedAddress } from '@solana/addresses';
import { PUBLIC_KEY_LENGTH } from '@solana/web3.js';
import { getMintSize } from '@solana-program/token-2022';

export const TOKEN_WRAP_PROGRAM_ID = address('TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR');
export const MINT_SIZE = BigInt(getMintSize());
export const BACKPOINTER_SIZE = BigInt(PUBLIC_KEY_LENGTH);

export const getWrappedMintAddress = async (
  unwrappedMint: Address,
  wrappedTokenProgramId: Address,
): Promise<Address> => {
  const addressEncoder = getAddressEncoder();
  const [pda] = await getProgramDerivedAddress({
    programAddress: TOKEN_WRAP_PROGRAM_ID,
    seeds: [
      new TextEncoder().encode('mint'),
      addressEncoder.encode(unwrappedMint),
      addressEncoder.encode(wrappedTokenProgramId),
    ],
  });
  return pda;
};

export const getBackpointerAddress = async (wrappedMint: Address): Promise<Address> => {
  const addressEncoder = getAddressEncoder();
  const [pda] = await getProgramDerivedAddress({
    programAddress: TOKEN_WRAP_PROGRAM_ID,
    seeds: [new TextEncoder().encode('backpointer'), addressEncoder.encode(wrappedMint)],
  });
  return pda;
};
