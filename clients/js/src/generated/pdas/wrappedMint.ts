/**
 * This code was AUTOGENERATED using the codama library.
 * Please DO NOT EDIT THIS FILE, instead use visitors
 * to add features, then rerun codama to update it.
 *
 * @see https://github.com/codama-idl/codama
 */

import {
  getAddressEncoder,
  getProgramDerivedAddress,
  getUtf8Encoder,
  type Address,
  type ProgramDerivedAddress,
} from '@solana/kit';

export type WrappedMintSeeds = {
  unwrappedMint: Address;

  wrappedTokenProgram: Address;
};

export async function findWrappedMintPda(
  seeds: WrappedMintSeeds,
  config: { programAddress?: Address | undefined } = {}
): Promise<ProgramDerivedAddress> {
  const {
    programAddress = 'TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR' as Address<'TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR'>,
  } = config;
  return await getProgramDerivedAddress({
    programAddress,
    seeds: [
      getUtf8Encoder().encode('mint'),
      getAddressEncoder().encode(seeds.unwrappedMint),
      getAddressEncoder().encode(seeds.wrappedTokenProgram),
    ],
  });
}
