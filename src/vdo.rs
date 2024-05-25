// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! The VDO data structures

use proc_macros::CApiWrapper;
use proc_macros::Printf;
use proc_macros::Snprintf;

use crate::pd::pd3p2::vdo::CertStat;
use crate::pd::pd3p2::vdo::Dfp;
use crate::pd::pd3p2::vdo::IdHeader;

#[cfg(feature = "c_api")]
pub(crate) mod c_api {
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoCertStat;
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoDfp;
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoIdHeader;
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoProductType;
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoUfp;
    pub(crate) use crate::pd::pd3p2::vdo::Pd3p2VdoVpd;
}

#[cfg(feature = "c_api")]
pub(crate) use c_api::*;

use crate::pd::pd3p2::vdo::ProductType;
use crate::pd::pd3p2::vdo::Ufp;
use crate::pd::pd3p2::vdo::Vpd;

#[derive(Debug, Clone, PartialEq, CApiWrapper)]
#[c_api(prefix = "TypeCRs", repr_c = true)]
/// A type representing the different types of VDO supported by the library.
pub enum Vdo {
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2IdHeader(IdHeader),
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2CertStat(CertStat),
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2ProductType(ProductType),
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2Vpd(Vpd),
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2Ufp(Ufp),
    #[c_api(variant_prefix = "Pd3p2Vdo")]
    Pd3p2Dfp(Dfp),
}
