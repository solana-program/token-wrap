import fs from "node:fs";
import {
  accountNode,
  constantPdaSeedNodeFromString,
  createFromRoot,
  pdaLinkNode,
  pdaNode,
  programNode,
  publicKeyTypeNode,
  rootNode,
  structFieldTypeNode,
  structTypeNode,
  variablePdaSeedNode,
} from "codama";

const codama = createFromRoot(
  rootNode(
    programNode({
      name: "tokenWrap",
      publicKey: "TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR",
      version: "0.1.0",
      accounts: [
        accountNode({
          name: "backpointer",
          data: structTypeNode([
            structFieldTypeNode({
              name: "unwrappedMint",
              type: publicKeyTypeNode(),
            }),
          ]),
          pda: pdaLinkNode("backpointer"),
        }),
      ],
      instructions: [],
      definedTypes: [],
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
      errors: [],
    })
  )
);

fs.writeFileSync("program/idl.json", JSON.stringify(codama.getRoot(), null, 2));
