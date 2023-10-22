macro_rules! auto_map {
    ($t1:ident $t2:ident {$(($value1:path,$value2:path)), +}) => {
        impl TryFrom<$t1> for $t2 {
            type Error = anyhow::Error;
            #[cfg_attr(feature = "profiling", profiling::function)]
            fn try_from(f: $t1) -> Result<$t2, anyhow::Error> {
                #[allow(unreachable_patterns)]
                match f {
                    $( $value1 => {
                        Ok($value2)
                    } )*
                    _ => {
                        anyhow::bail!("Failed to map {:?} to {:?}", f, stringify!($t1))
                    }
                }
            }
        }

        impl TryFrom<$t2> for $t1 {
            type Error = anyhow::Error;
            #[cfg_attr(feature = "profiling", profiling::function)]
            fn try_from(f: $t2) -> Result<$t1, anyhow::Error> {
                #[allow(unreachable_patterns)]
                match f {
                    $( $value2 => {
                        Ok($value1)
                    } )*
                    _ => {
                        anyhow::bail!("Failed to map {:?} to {:?}", f, stringify!($t2))
                    }
                }
            }
        }
    };
}
pub(crate) use auto_map;
