// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! USB Power Delivery (PD) functionality.
//!
//! See "Universal Serial Bus Power Delivery Specification"

use bitstream_io::BitRead;
use enumn::N;
use proc_macros::CApiWrapper;
use proc_macros::Printf;
use proc_macros::Snprintf;

use crate::pd::pd3p2::BatterySupplyPdo;
use crate::pd::pd3p2::FixedSupplyPdo;
use crate::BcdWrapper;
use crate::BitReader;
use crate::Error;
use crate::FromBytes;
use crate::Result;

use crate::pd::pd3p2::BatteryCapData;
use crate::pd::pd3p2::BatteryStatusData;
use crate::pd::pd3p2::DiscoverIdentityResponse;
use crate::pd::pd3p2::Pd3p2BatteryCapData;
use crate::pd::pd3p2::Pd3p2BatteryStatusData;
use crate::pd::pd3p2::Pd3p2BatterySupplyPdo;
use crate::pd::pd3p2::Pd3p2DiscoverIdentityResponse;
use crate::pd::pd3p2::Pd3p2FixedSupplyPdo;
use crate::pd::pd3p2::Pd3p2RevisionMessageData;
use crate::pd::pd3p2::Pd3p2SinkCapabilitiesExtended;
use crate::pd::pd3p2::Pd3p2SourceCapabilitiesExtended;
use crate::pd::pd3p2::Pd3p2SprProgrammableSupplyPdo;
use crate::pd::pd3p2::Pd3p2VariableSupplyPdo;
use crate::pd::pd3p2::RevisionMessageData;
use crate::pd::pd3p2::SinkCapabilitiesExtended;
use crate::pd::pd3p2::SourceCapabilitiesExtended;
use crate::pd::pd3p2::SprProgrammableSupplyPdo;
use crate::pd::pd3p2::VariableSupplyPdo;

pub mod pd3p2;

#[derive(Debug, Clone, PartialEq, Default, N, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum CommandType {
    /// Request from initiator port.
    #[default]
    Request,
    /// Acknowledge response from responder port.
    Ack,
    /// Negative acknowledge response from responder port.
    Nak,
    /// Busy response from responder port.
    Busy,
}

#[derive(Debug, Clone, PartialEq, Default, N, Copy, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum Command {
    /// The Discover Identity Command is provided to enable an Initiator to
    /// identify its Port Partner and for an Initiator (VCONN Source) to
    /// identify the Responder (Cable Plug or VPD). The Discover Identity
    /// Command is also used to determine whether a Cable Plug or VPD is
    /// PD-Capable by looking for a GoodCRC Message Response.
    #[default]
    DiscoverIdentity,
    DiscoverSVIDs,
    DiscoverModes,
    EnterMode,
    ExitMode,
    Attention,
    SVIDSpecific,
}

#[derive(Debug, Clone, PartialEq, Default, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
/// The VDM header. See table 6.30 in the USB PD Specification for more
/// information.
pub struct VdmHeader {
    // Whether this is a structured VDM.
    pub structured: bool,
    // The major version number of this VDM.
    pub major: u8,
    // Them minor major version number of this VDM.
    pub minor: u8,
    /// For Enter Mode, Exit Mode and Attention commands:
    ///
    /// Index into the list of VDOs to identify the desired Mode
    ///
    /// For Exit Mode only: 0b111 to exit all Active Modes
    ///
    /// Zero otherwise.
    pub object_position: u8,
    /// The command type.
    pub command_type: CommandType,
    /// The command.
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum Pdo {
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2FixedSupplyPdo(FixedSupplyPdo),
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2BatterySupplyPdo(BatterySupplyPdo),
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2VariableSupplyPdo(VariableSupplyPdo),
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2AugmentedPdo(SprProgrammableSupplyPdo),
}

impl Pdo {
    pub fn from_bytes(reader: &mut BitReader, revision: BcdWrapper) -> Result<Self> {
        // See USB PD 3.2. - Table 6.7 “Power Data Object”
        let pdo_type = reader.read::<u32>(2)?;
        match pdo_type {
            0 => match revision.0 {
                0x310 => {
                    let pdo = FixedSupplyPdo::from_bytes(reader)?;
                    Ok(Pdo::Pd3p2FixedSupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            1 => match revision.0 {
                0x310 => {
                    let pdo = BatterySupplyPdo::from_bytes(reader)?;
                    Ok(Pdo::Pd3p2BatterySupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            2 => match revision.0 {
                0x310 => {
                    let pdo = VariableSupplyPdo::from_bytes(reader)?;
                    Ok(Pdo::Pd3p2VariableSupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            3 => match revision.0 {
                0x310 => {
                    let pdo = SprProgrammableSupplyPdo::from_bytes(reader)?;
                    Ok(Pdo::Pd3p2AugmentedPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            other => Err(Error::ParseError {
                field: "pdo_type (i.e.: bits31..30)".into(),
                value: other,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum Message {
    /// Sink Capabilities Extended (Extended Message)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2SinkCapabilitiesExtended(SinkCapabilitiesExtended),
    /// Source Capabilities Extended (Extended Message)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2SourceCapabilitiesExtended(SourceCapabilitiesExtended),
    /// Battery Capabilities (Extended Message)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2BatteryCapabilities(BatteryCapData),
    /// Battery Status (Data Message)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2BatteryStatus(BatteryStatusData),
    /// Discover Identity Response – ACK, NAK or BUSY (Structured VDM)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2DiscoverIdentityResponse(DiscoverIdentityResponse),
    /// Revision (Data Message)
    #[c_api(variant_prefix = "Pd3p2")]
    Pd3p2Revision(RevisionMessageData),
}

/// This enum represents the recipient of the PD message.
#[derive(Debug, Clone, PartialEq, Default, N, Copy, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum PdMessageRecipient {
    #[default]
    /// The OPM wants to retrieve the USB PD response message from the
    /// identified connector.
    Connector,
    /// The OPM wants to retrieve the USB PD response message from the port
    /// partner of the identified connector.
    Sop,
    /// The OPM wants to retrieve the USB PD response message from the cable
    /// plug of the identified connector.
    SopPrime,
    /// The OPM wants to retrieve the USB PD response message from the cable
    /// plug of the identified connector.
    SopDoublePrime,
}

/// This enum represents the type of the PD response message.
#[derive(Debug, Clone, PartialEq, Default, N, Copy, CApiWrapper)]
#[c_api(prefix = "Pd", repr_c = true)]
pub enum PdMessageResponseType {
    #[default]
    /// Sink Capabilities Extended (Extended Message)
    SinkCapabilitiesExtended,
    /// Source Capabilities Extended (Extended Message)
    SourceCapabilitiesExtended,
    /// Battery Capabilities (Extended Message)
    BatteryCapabilities,
    /// Battery Status (Data Message)
    BatteryStatus,
    /// Discover Identity Response – ACK, NAK or BUSY (Structured VDM)
    DiscoverIdentity,
    /// Revision (Data Message)
    Revision,
    /// Reserved values.
    Reserved,
}
