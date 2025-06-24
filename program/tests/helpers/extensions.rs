use {
    solana_pubkey::Pubkey,
    spl_pod::{optional_keys::OptionalNonZeroPubkey, primitives::PodU64},
    spl_token_2022::{
        extension::{
            immutable_owner::ImmutableOwner,
            transfer_fee::{TransferFee, TransferFeeAmount, TransferFeeConfig},
            transfer_hook::{TransferHook, TransferHookAccount},
            BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut,
        },
        pod::{PodAccount, PodMint},
    },
};

/// Initialize extensions for a mint account
pub fn init_mint_extensions(
    state: &mut PodStateWithExtensionsMut<PodMint>,
    extensions: &[ExtensionType],
) {
    for extension in extensions {
        match extension {
            ExtensionType::TransferHook => {
                let extension = state.init_extension::<TransferHook>(false).unwrap();
                extension.program_id =
                    OptionalNonZeroPubkey::try_from(Some(test_transfer_hook::id())).unwrap();
            }
            ExtensionType::TransferFeeConfig => {
                let extension = state.init_extension::<TransferFeeConfig>(false).unwrap();
                *extension = TransferFeeConfig {
                    transfer_fee_config_authority: OptionalNonZeroPubkey::try_from(Some(
                        Pubkey::new_unique(),
                    ))
                    .unwrap(),
                    withdraw_withheld_authority: OptionalNonZeroPubkey::try_from(Some(
                        Pubkey::new_unique(),
                    ))
                    .unwrap(),
                    withheld_amount: PodU64::from(0),
                    older_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: 50_000.into(),
                        transfer_fee_basis_points: 100.into(),
                    },
                    newer_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: 50_000.into(),
                        transfer_fee_basis_points: 100.into(),
                    },
                };
            }
            _ => unimplemented!(),
        }
    }
}

/// Initialize extensions for a token account
pub fn init_token_account_extensions(
    state: &mut PodStateWithExtensionsMut<PodAccount>,
    extensions: &[ExtensionType],
) {
    for extension in extensions {
        match extension {
            ExtensionType::ImmutableOwner => {
                state.init_extension::<ImmutableOwner>(true).unwrap();
            }
            ExtensionType::TransferFeeAmount => {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
            }
            ExtensionType::TransferFeeConfig => {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
            }
            ExtensionType::TransferHookAccount => {
                state.init_extension::<TransferHookAccount>(true).unwrap();
            }
            _ => unimplemented!(),
        }
    }
}
