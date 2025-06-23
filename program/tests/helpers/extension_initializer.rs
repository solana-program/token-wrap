use spl_token_2022::extension::{BaseState, ExtensionType, PodStateWithExtensionsMut};

pub trait ExtensionInitializer<T: BaseState> {
    fn extension_type(&self) -> ExtensionType;
    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<T>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}
