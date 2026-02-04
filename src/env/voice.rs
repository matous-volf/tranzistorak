load_dotenv::load_dotenv!();

pub(crate) const COMMANDS_ENABLED: &str = env!("VOICE_COMMANDS_ENABLED");
pub(crate) const MODEL_NAME: &str = env!("VOICE_MODEL_NAME");
pub(crate) const PREPROCESSOR_CONFIG_MODEL_NAME: &str =
    env!("VOICE_PREPROCESSOR_CONFIG_MODEL_NAME");
