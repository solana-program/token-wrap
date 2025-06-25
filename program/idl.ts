import {
  accountNode,
  argumentValueNode,
  booleanTypeNode,
  booleanValueNode,
  constantPdaSeedNodeFromString,
  createFromRoot,
  errorNode,
  fieldDiscriminatorNode,
  identityValueNode,
  instructionAccountNode,
  instructionArgumentNode,
  instructionNode,
  instructionRemainingAccountsNode,
  numberTypeNode,
  numberValueNode,
  pdaLinkNode,
  pdaNode,
  programNode,
  publicKeyTypeNode,
  publicKeyValueNode,
  rootNode,
  structFieldTypeNode,
  structTypeNode,
  variablePdaSeedNode,
} from "codama";
import { writeFileSync } from "fs";
import { SYSTEM_PROGRAM_ADDRESS } from "@solana-program/system";

// Note: this is temporary until Codama macros are available: https://github.com/codama-idl/codama-rs

const codama = createFromRoot(
  rootNode(
    programNode({
      name: "tokenWrap",
      publicKey: "TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR",
      version: "0.1.0",
      accounts: [
        accountNode({
          name: "backpointer",
          docs: "Account to store the address of the unwrapped mint.",
          data: structTypeNode([
            structFieldTypeNode({
              name: "unwrappedMint",
              type: publicKeyTypeNode(),
            }),
          ]),
          pda: pdaLinkNode("backpointer"),
          size: 32,
        }),
      ],
      instructions: [
        instructionNode({
          name: "createMint",
          docs: [
            "Create a wrapped token mint. Assumes caller has pre-funded wrapped mint",
            "and backpointer account. Supports both directions:",
            "- spl-token to token-2022",
            "- token-2022 to spl-token",
            "- token-2022 to token-2022 w/ new extensions",
          ],
          accounts: [
            instructionAccountNode({
              name: "wrappedMint",
              docs: [
                " Unallocated wrapped mint account to create (PDA), address must be:",
                "`get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "backpointer",
              docs: [
                "Unallocated wrapped backpointer account to create (PDA)",
                "`get_wrapped_mint_backpointer_address(wrapped_mint_address)`",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "unwrappedMint",
              docs: "The existing mint",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "systemProgram",
              docs: "The system program",
              isSigner: false,
              isWritable: false,
              isOptional: true,
              defaultValue: publicKeyValueNode(SYSTEM_PROGRAM_ADDRESS),
            }),
            instructionAccountNode({
              name: "wrappedTokenProgram",
              docs: "The token program used to create the wrapped mint",
              isSigner: false,
              isWritable: false,
            }),
          ],
          discriminators: [fieldDiscriminatorNode("discriminator", 0)],
          arguments: [
            instructionArgumentNode({
              name: "discriminator",
              type: numberTypeNode("u8"),
              defaultValue: numberValueNode(0),
              defaultValueStrategy: "omitted",
            }),
            instructionArgumentNode({
              name: "idempotent",
              docs: "Whether the creation should fail if the wrapped mint already exists.",
              type: booleanTypeNode(),
              defaultValue: booleanValueNode(false),
              defaultValueStrategy: "optional",
            }),
          ],
        }),
        instructionNode({
          name: "wrap",
          docs: [
            "Move a user's unwrapped tokens into an escrow account and mint the same",
            "number of wrapped tokens into the provided account.",
          ],
          accounts: [
            instructionAccountNode({
              name: "recipientWrappedTokenAccount",
              docs: "The token account to receive the wrapped tokens.",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "wrappedMint",
              docs: [
                "Wrapped mint, must be initialized, address must be:",
                "`get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "wrappedMintAuthority",
              docs: [
                "The PDA authority of the wrapped mint, address must be",
                "`get_wrapped_mint_authority(wrapped_mint)`",
              ],
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "unwrappedTokenProgram",
              docs: "The token program of the unwrapped tokens.",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "wrappedTokenProgram",
              docs: "The token program of the wrapped tokens.",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "unwrappedTokenAccount",
              docs: "The source token account for the unwrapped tokens.",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "unwrappedMint",
              docs: "The mint of the unwrapped tokens.",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "unwrappedEscrow",
              docs: [
                "The escrow account that holds the unwrapped tokens.",
                "Address must be ATA: get_escrow_address(unwrapped_mint, unwrapped_token_program, wrapped_token_program)",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "transferAuthority",
              docs: "The authority to transfer the unwrapped tokens.",
              isSigner: "either",
              isWritable: false,
              defaultValue: identityValueNode(),
            }),
          ],
          remainingAccounts: [
            instructionRemainingAccountsNode(
              argumentValueNode("multiSigners"),
              { isSigner: true, isOptional: true },
            ),
          ],
          discriminators: [fieldDiscriminatorNode("discriminator", 0)],
          arguments: [
            instructionArgumentNode({
              name: "discriminator",
              type: numberTypeNode("u8"),
              defaultValue: numberValueNode(1),
              defaultValueStrategy: "omitted",
            }),
            instructionArgumentNode({
              name: "amount",
              docs: "The amount of tokens to wrap.",
              type: numberTypeNode("u64"),
            }),
          ],
        }),
        instructionNode({
          name: "unwrap",
          docs: "Burns wrapped tokens and releases unwrapped tokens from the escrow account.",
          accounts: [
            instructionAccountNode({
              name: "unwrappedEscrow",
              docs: [
                "The escrow account holding the unwrapped tokens.",
                "Address must be ATA: get_escrow_address(unwrapped_mint, unwrapped_token_program, wrapped_token_program)",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "recipientUnwrappedToken",
              docs: "The account to receive the unwrapped tokens.",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "wrappedMintAuthority",
              docs: [
                "The PDA authority of the wrapped mint,",
                "address must be: `get_wrapped_mint_authority(wrapped_mint)`",
              ],
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "unwrappedMint",
              docs: "The mint of the unwrapped tokens",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "wrappedTokenProgram",
              docs: "The token program of the wrapped tokens",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "unwrappedTokenProgram",
              docs: "The token program of the unwrapped tokens",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "wrappedTokenAccount",
              docs: "The source token account for the wrapped tokens",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "wrappedMint",
              docs: [
                "The wrapped mint account, address must be:",
                "`get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`",
              ],
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "transferAuthority",
              docs: "The authority to burn the wrapped tokens.",
              isSigner: "either",
              isWritable: false,
              defaultValue: identityValueNode(),
            }),
          ],
          remainingAccounts: [
            instructionRemainingAccountsNode(
              argumentValueNode("multiSigners"),
              { isSigner: true, isOptional: true },
            ),
          ],
          discriminators: [fieldDiscriminatorNode("discriminator", 0)],
          arguments: [
            instructionArgumentNode({
              name: "discriminator",
              type: numberTypeNode("u8"),
              defaultValue: numberValueNode(2),
              defaultValueStrategy: "omitted",
            }),
            instructionArgumentNode({
              name: "amount",
              docs: "The amount of tokens to unwrap.",
              type: numberTypeNode("u64"),
            }),
          ],
        }),
        instructionNode({
          name: "closeStuckEscrow",
          docs: [
            "Closes a stuck escrow `ATA`. This is for the edge case where an",
            "unwrapped mint with a close authority is closed and then a new mint",
            "is created at the same address but with a different size, leaving",
            "the escrow `ATA` in a bad state.",
            "This instruction will close the old escrow `ATA`, returning the lamports",
            "to the destination account. It will only work if the current escrow has",
            "different extensions than the mint. The client is then responsible",
            "for calling `create_associated_token_account` to recreate it.",
          ],
          accounts: [
            instructionAccountNode({
              name: "escrow",
              docs: "Escrow account to close (ATA)",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "destination",
              docs: "Destination for lamports from closed account",
              isSigner: false,
              isWritable: true,
            }),
            instructionAccountNode({
              name: "unwrappedMint",
              docs: "Unwrapped mint",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "wrappedMint",
              docs: "Wrapped mint",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "wrappedMintAuthority",
              docs: "Wrapped mint authority (PDA)",
              isSigner: false,
              isWritable: false,
            }),
            instructionAccountNode({
              name: "token2022Program",
              docs: "Token-2022 program",
              isSigner: false,
              isWritable: false,
              isOptional: true,
              defaultValue: publicKeyValueNode(
                "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
              ),
            }),
          ],
          discriminators: [fieldDiscriminatorNode("discriminator", 0)],
          arguments: [
            instructionArgumentNode({
              name: "discriminator",
              type: numberTypeNode("u8"),
              defaultValue: numberValueNode(3),
              defaultValueStrategy: "omitted",
            }),
          ],
        }),
      ],
      pdas: [
        pdaNode({
          name: "backpointer",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "backpointer"),
            variablePdaSeedNode("wrappedMint", publicKeyTypeNode()),
          ],
        }),
        pdaNode({
          name: "wrappedMint",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "mint"),
            variablePdaSeedNode("unwrappedMint", publicKeyTypeNode()),
            variablePdaSeedNode("wrappedTokenProgram", publicKeyTypeNode()),
          ],
        }),
        pdaNode({
          name: "wrappedMintAuthority",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "authority"),
            variablePdaSeedNode("wrappedMint", publicKeyTypeNode()),
          ],
        }),
      ],
      errors: [
        errorNode({
          name: "WrappedMintMismatch",
          code: 0,
          message: "Wrapped mint account address does not match expected PDA",
        }),
        errorNode({
          name: "BackpointerMismatch",
          code: 1,
          message:
            "Wrapped backpointer account address does not match expected PDA",
        }),
        errorNode({
          name: "ZeroWrapAmount",
          code: 2,
          message: "Wrap amount should be positive",
        }),
        errorNode({
          name: "MintAuthorityMismatch",
          code: 3,
          message: "Wrapped mint authority does not match expected PDA",
        }),
        errorNode({
          name: "EscrowOwnerMismatch",
          code: 4,
          message: "Unwrapped escrow token owner is not set to expected PDA",
        }),
        errorNode({
          name: "InvalidWrappedMintOwner",
          code: 5,
          message:
            "Wrapped mint account owner is not the expected token program",
        }),
        errorNode({
          name: "InvalidBackpointerOwner",
          code: 6,
          message:
            "Wrapped backpointer account owner is not the expected token wrap program",
        }),
        errorNode({
          name: "EscrowMismatch",
          code: 7,
          message: "Escrow account address does not match expected ATA",
        }),
        errorNode({
          name: "EscrowInGoodState",
          code: 8,
          message:
            "The escrow account is in a good state and cannot be recreated",
        }),
      ],
    }),
  ),
);

writeFileSync("program/idl.json", JSON.stringify(codama.getRoot(), null, 2));
