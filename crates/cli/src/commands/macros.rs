macro_rules! setup_commands {
    {
        $(
            #![run($($arg_name:ident: $arg_ty:ty),*$(,)?)]
        )?
        $(
            $([cfg($($cfg_tt:tt)*)])?
            $(#[$variant_meta:meta])*
            $variant_name:ident($mod_name:ident $(, $($pass_arg_name:ident),*$(,)?)?)
        ),*$(,)?
    } => {
        $(
            $(#[cfg($($cfg_tt)*)])?
            mod $mod_name;
        )*

        #[derive(Debug, Clone, clap::Parser)]
        pub enum Command {
            $(
                $(#[cfg($($cfg_tt)*)])?
                $(#[$variant_meta])*
                $variant_name($mod_name::Options)
            ),*
        }

        impl Command {
            pub async fn run(self, $($($arg_name: $arg_ty),*)?) -> anyhow::Result<()> {
                match self {
                    $(
                        $(#[cfg($($cfg_tt)*)])?
                        Self::$variant_name(opts) => $mod_name::run(opts, $($($pass_arg_name),*)?).await
                    ),*
                }
            }
        }
    };
}

macro_rules! setup_command {
    {
        $(
            $(#[$field_meta:meta])*
            $field_name:ident: $field_ty:ty
        ),*$(,)?
    } => {
        #[derive(Debug, Clone, clap::Parser)]
        pub struct Options {
            $(
                $(#[$field_meta])*
                $field_name: $field_ty
            ),*
        }
    };
}
