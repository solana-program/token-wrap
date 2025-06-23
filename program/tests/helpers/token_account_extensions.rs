use {
    crate::helpers::extension_initializer::ExtensionInitializer,
    spl_pod::primitives::PodBool,
    spl_token_2022::{
        extension::{
            immutable_owner::ImmutableOwner, transfer_fee::TransferFeeAmount,
            transfer_hook::TransferHookAccount, BaseStateWithExtensionsMut, ExtensionType,
            PodStateWithExtensionsMut,
        },
        pod::PodAccount,
    },
};

pub struct ImmutableOwnerExtension;

impl ExtensionInitializer<PodAccount> for ImmutableOwnerExtension {
    fn extension_type(&self) -> ExtensionType {
        ExtensionType::ImmutableOwner
    }

    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<PodAccount>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        state.init_extension::<ImmutableOwner>(true)?;
        Ok(())
    }
}

pub struct TransferFeeAmountExtension {
    pub withheld_amount: u64,
}

impl TransferFeeAmountExtension {
    pub fn new(withheld_amount: u64) -> Self {
        Self { withheld_amount }
    }
}

impl ExtensionInitializer<PodAccount> for TransferFeeAmountExtension {
    fn extension_type(&self) -> ExtensionType {
        ExtensionType::TransferFeeAmount
    }

    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<PodAccount>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension = state.init_extension::<TransferFeeAmount>(true)?;
        extension.withheld_amount = self.withheld_amount.into();
        Ok(())
    }
}

pub struct TransferHookAccountExtension {
    pub transferring: bool,
}

impl TransferHookAccountExtension {
    pub fn new(transferring: bool) -> Self {
        Self { transferring }
    }
}

impl ExtensionInitializer<PodAccount> for TransferHookAccountExtension {
    fn extension_type(&self) -> ExtensionType {
        ExtensionType::TransferHookAccount
    }

    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<PodAccount>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension = state.init_extension::<TransferHookAccount>(true)?;
        extension.transferring = PodBool::from_bool(self.transferring);
        Ok(())
    }
}
