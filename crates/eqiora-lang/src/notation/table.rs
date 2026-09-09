//! The complete initial notation command table; aliases are deliberately not inferred.

/// Admitted letter or distinguished symbol. Digits occur only in intrinsic scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotationAtom {
    /// ASCII Latin letter with its exact case.
    Latin(char),
    /// One decimal digit in an intrinsic script.
    Digit(u8),
    /// The `\\alpha` atom.
    Alpha,
    /// The `\\Alpha` atom.
    AlphaUpper,
    /// The `\\beta` atom.
    Beta,
    /// The `\\Beta` atom.
    BetaUpper,
    /// The `\\gamma` atom.
    Gamma,
    /// The `\\Gamma` atom.
    GammaUpper,
    /// The `\\delta` atom.
    Delta,
    /// The `\\Delta` atom.
    DeltaUpper,
    /// The `\\epsilon` atom.
    Epsilon,
    /// The `\\Epsilon` atom.
    EpsilonUpper,
    /// The `\\zeta` atom.
    Zeta,
    /// The `\\Zeta` atom.
    ZetaUpper,
    /// The `\\eta` atom.
    Eta,
    /// The `\\Eta` atom.
    EtaUpper,
    /// The `\\theta` atom.
    Theta,
    /// The `\\Theta` atom.
    ThetaUpper,
    /// The `\\iota` atom.
    Iota,
    /// The `\\Iota` atom.
    IotaUpper,
    /// The `\\kappa` atom.
    Kappa,
    /// The `\\Kappa` atom.
    KappaUpper,
    /// The `\\lambda` atom.
    Lambda,
    /// The `\\Lambda` atom.
    LambdaUpper,
    /// The `\\mu` atom.
    Mu,
    /// The `\\Mu` atom.
    MuUpper,
    /// The `\\nu` atom.
    Nu,
    /// The `\\Nu` atom.
    NuUpper,
    /// The `\\xi` atom.
    Xi,
    /// The `\\Xi` atom.
    XiUpper,
    /// The `\\omicron` atom.
    Omicron,
    /// The `\\Omicron` atom.
    OmicronUpper,
    /// The `\\pi` atom.
    Pi,
    /// The `\\Pi` atom.
    PiUpper,
    /// The `\\rho` atom.
    Rho,
    /// The `\\Rho` atom.
    RhoUpper,
    /// The `\\sigma` atom.
    Sigma,
    /// The `\\Sigma` atom.
    SigmaUpper,
    /// The `\\tau` atom.
    Tau,
    /// The `\\Tau` atom.
    TauUpper,
    /// The `\\upsilon` atom.
    Upsilon,
    /// The `\\Upsilon` atom.
    UpsilonUpper,
    /// The `\\phi` atom.
    Phi,
    /// The `\\Phi` atom.
    PhiUpper,
    /// The `\\chi` atom.
    Chi,
    /// The `\\Chi` atom.
    ChiUpper,
    /// The `\\psi` atom.
    Psi,
    /// The `\\Psi` atom.
    PsiUpper,
    /// The `\\omega` atom.
    Omega,
    /// The `\\Omega` atom.
    OmegaUpper,
    /// The `\\varepsilon` atom.
    VariantEpsilon,
    /// The `\\vartheta` atom.
    VariantTheta,
    /// The `\\varkappa` atom.
    VariantKappa,
    /// The `\\varpi` atom.
    VariantPi,
    /// The `\\varrho` atom.
    VariantRho,
    /// The `\\varsigma` atom.
    VariantSigma,
    /// The `\\varphi` atom.
    VariantPhi,
    /// The `\\hbar` atom.
    Hbar,
    /// The `\\ell` atom.
    Ell,
    /// The `\\aleph` atom.
    Aleph,
}
impl NotationAtom {
    pub(super) fn command(command: &str) -> Option<Self> {
        Some(match command {
            "alpha" => Self::Alpha,
            "Alpha" => Self::AlphaUpper,
            "beta" => Self::Beta,
            "Beta" => Self::BetaUpper,
            "gamma" => Self::Gamma,
            "Gamma" => Self::GammaUpper,
            "delta" => Self::Delta,
            "Delta" => Self::DeltaUpper,
            "epsilon" => Self::Epsilon,
            "Epsilon" => Self::EpsilonUpper,
            "zeta" => Self::Zeta,
            "Zeta" => Self::ZetaUpper,
            "eta" => Self::Eta,
            "Eta" => Self::EtaUpper,
            "theta" => Self::Theta,
            "Theta" => Self::ThetaUpper,
            "iota" => Self::Iota,
            "Iota" => Self::IotaUpper,
            "kappa" => Self::Kappa,
            "Kappa" => Self::KappaUpper,
            "lambda" => Self::Lambda,
            "Lambda" => Self::LambdaUpper,
            "mu" => Self::Mu,
            "Mu" => Self::MuUpper,
            "nu" => Self::Nu,
            "Nu" => Self::NuUpper,
            "xi" => Self::Xi,
            "Xi" => Self::XiUpper,
            "omicron" => Self::Omicron,
            "Omicron" => Self::OmicronUpper,
            "pi" => Self::Pi,
            "Pi" => Self::PiUpper,
            "rho" => Self::Rho,
            "Rho" => Self::RhoUpper,
            "sigma" => Self::Sigma,
            "Sigma" => Self::SigmaUpper,
            "tau" => Self::Tau,
            "Tau" => Self::TauUpper,
            "upsilon" => Self::Upsilon,
            "Upsilon" => Self::UpsilonUpper,
            "phi" => Self::Phi,
            "Phi" => Self::PhiUpper,
            "chi" => Self::Chi,
            "Chi" => Self::ChiUpper,
            "psi" => Self::Psi,
            "Psi" => Self::PsiUpper,
            "omega" => Self::Omega,
            "Omega" => Self::OmegaUpper,
            "varepsilon" => Self::VariantEpsilon,
            "vartheta" => Self::VariantTheta,
            "varkappa" => Self::VariantKappa,
            "varpi" => Self::VariantPi,
            "varrho" => Self::VariantRho,
            "varsigma" => Self::VariantSigma,
            "varphi" => Self::VariantPhi,
            "hbar" => Self::Hbar,
            "ell" => Self::Ell,
            "aleph" => Self::Aleph,
            _ => return None,
        })
    }
    pub(super) fn spelling(self) -> String {
        match self {
            Self::Latin(letter) => letter.to_string(),
            Self::Digit(digit) => char::from(b'0' + digit).to_string(),
            Self::Alpha => "\\alpha".into(),
            Self::AlphaUpper => "\\Alpha".into(),
            Self::Beta => "\\beta".into(),
            Self::BetaUpper => "\\Beta".into(),
            Self::Gamma => "\\gamma".into(),
            Self::GammaUpper => "\\Gamma".into(),
            Self::Delta => "\\delta".into(),
            Self::DeltaUpper => "\\Delta".into(),
            Self::Epsilon => "\\epsilon".into(),
            Self::EpsilonUpper => "\\Epsilon".into(),
            Self::Zeta => "\\zeta".into(),
            Self::ZetaUpper => "\\Zeta".into(),
            Self::Eta => "\\eta".into(),
            Self::EtaUpper => "\\Eta".into(),
            Self::Theta => "\\theta".into(),
            Self::ThetaUpper => "\\Theta".into(),
            Self::Iota => "\\iota".into(),
            Self::IotaUpper => "\\Iota".into(),
            Self::Kappa => "\\kappa".into(),
            Self::KappaUpper => "\\Kappa".into(),
            Self::Lambda => "\\lambda".into(),
            Self::LambdaUpper => "\\Lambda".into(),
            Self::Mu => "\\mu".into(),
            Self::MuUpper => "\\Mu".into(),
            Self::Nu => "\\nu".into(),
            Self::NuUpper => "\\Nu".into(),
            Self::Xi => "\\xi".into(),
            Self::XiUpper => "\\Xi".into(),
            Self::Omicron => "\\omicron".into(),
            Self::OmicronUpper => "\\Omicron".into(),
            Self::Pi => "\\pi".into(),
            Self::PiUpper => "\\Pi".into(),
            Self::Rho => "\\rho".into(),
            Self::RhoUpper => "\\Rho".into(),
            Self::Sigma => "\\sigma".into(),
            Self::SigmaUpper => "\\Sigma".into(),
            Self::Tau => "\\tau".into(),
            Self::TauUpper => "\\Tau".into(),
            Self::Upsilon => "\\upsilon".into(),
            Self::UpsilonUpper => "\\Upsilon".into(),
            Self::Phi => "\\phi".into(),
            Self::PhiUpper => "\\Phi".into(),
            Self::Chi => "\\chi".into(),
            Self::ChiUpper => "\\Chi".into(),
            Self::Psi => "\\psi".into(),
            Self::PsiUpper => "\\Psi".into(),
            Self::Omega => "\\omega".into(),
            Self::OmegaUpper => "\\Omega".into(),
            Self::VariantEpsilon => "\\varepsilon".into(),
            Self::VariantTheta => "\\vartheta".into(),
            Self::VariantKappa => "\\varkappa".into(),
            Self::VariantPi => "\\varpi".into(),
            Self::VariantRho => "\\varrho".into(),
            Self::VariantSigma => "\\varsigma".into(),
            Self::VariantPhi => "\\varphi".into(),
            Self::Hbar => "\\hbar".into(),
            Self::Ell => "\\ell".into(),
            Self::Aleph => "\\aleph".into(),
        }
    }
}

macro_rules! commands {
    ($name:ident, $doc:literal, $($variant:ident => $command:literal),+ $(,)?) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $(#[doc = $command] $variant),+ }
        impl $name {
            pub(super) fn command(command: &str) -> Option<Self> {
                Some(match command { $($command => Self::$variant,)+ _ => return None })
            }
            pub(super) const fn spelling(self) -> &'static str {
                match self { $(Self::$variant => concat!("\\", $command),)+ }
            }
        }
    };
}
commands!(NotationStyle, "Explicit symbol style, independent of the declaration's mathematical type.",
    Roman => "mathrm", Italic => "mathit", Bold => "mathbf", SansSerif => "mathsf",
    Monospace => "mathtt", Calligraphic => "mathcal", BlackboardBold => "mathbb",
);
commands!(NotationAccent, "An admitted symbol accent, without mathematical evaluation.",
    Hat => "hat", Tilde => "tilde", Bar => "bar", Vector => "vec", Dot => "dot", DoubleDot => "ddot",
);
commands!(NotationMark, "An intrinsic script mark, never an expression operation.",
    Prime => "prime", Star => "star", Transpose => "top", Dagger => "dagger",
    Plus => "plus", Minus => "minus", PlusMinus => "pm", MinusPlus => "mp",
);
