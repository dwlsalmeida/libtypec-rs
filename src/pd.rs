// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! USB Power Delivery (PD) functionality.
//!
//! See "Universal Serial Bus Power Delivery Specification"

use bitstream_io::BitRead;
use enumn::N;
use proc_macros::Printf;
use proc_macros::Snprintf;

use crate::vdo::Pd3p2CertStatVdo;
use crate::vdo::Pd3p2IdHeaderVdo;
use crate::vdo::Pd3p2ProductTypeVdo;
use crate::vdo::Pd3p2ProductVdo;
use crate::BcdWrapper;
use crate::BitReader;
use crate::Error;
use crate::FromBytes;
use crate::Milliamp;
use crate::Millivolt;
use crate::Milliwatt;
use crate::Result;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum PdCommandType {
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

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum PdCommand {
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

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
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
    pub command_type: PdCommandType,
    /// The command.
    pub command: PdCommand,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
/// The response to a Discover Identity command.
pub struct Pd3p2DiscoverIdentityResponse {
    pub header: VdmHeader,
    pub id_header_vdo: Pd3p2IdHeaderVdo,
    pub cert_stat: Pd3p2CertStatVdo,
    pub product_vdo: Pd3p2ProductVdo,
    pub product_type_vdo: [Pd3p2ProductTypeVdo; 3],
}

/// This enum represents the Touch Temperature.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub enum Pd3p2SceTouchTemp {
    NotApplicable = 0,
    Iec60950_1 = 1,
    Iec62368_1Ts1 = 2,
    Iec62368_1Ts2 = 3,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SceLoadStep {
    /// 150mA/µs Load Step (default)
    pub load_step_150ma: bool,
    /// 500mA/µs Load Step
    pub load_step_500ma: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SinkLoadCharacteristics {
    /// Percent overload in 10% increments. Values higher than 25 (11001b)
    /// are clipped to 250%. 00000b is the default.
    pub percent_overload: bool,
    /// Overload period in 20ms when bits 0-4 non-zero.
    pub overload_period: bool,
    /// Duty cycle in 5% increments when bits 0-4 are non-zero
    pub duty_cycle: bool,
    /// Can tolerate VBUS Voltage droop
    pub vbus_voltage_droop: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct SCEDCompliance {
    /// Requires LPS Source when set
    pub requires_lps_source: bool,
    /// Requires PS1 Source when set
    pub requires_ps1_source: bool,
    /// Requires PS2 Source when set
    pub requires_ps2_source: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct SCEDSinkModes {
    /// 1: PPS charging supported
    pub pps_charging_supported: bool,
    /// 1: VBUS powered
    pub vbus_powered: bool,
    /// 1: Mains powered
    pub mains_powered: bool,
    /// 1: Battery powered
    pub battery_powered: bool,
    /// 1: Battery essentially unlimited
    pub battery_essentially_unlimited: bool,
    /// 1: AVS Supported
    pub avs_supported: bool,
}

/// This struct represents the Sink Capabilities Extended Data.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SinkCapabilitiesExtended {
    /// Numeric Vendor ID (assigned by the USB-IF)
    pub vid: u32,
    /// Numeric Product ID (assigned by the manufacturer)
    pub pid: u32,
    /// Numeric Value provided by the USB-IF assigned to the product
    pub xid: u32,
    /// Numeric Firmware version number
    pub fw_version: u32,
    /// Numeric Hardware version number
    pub hw_version: u32,
    /// Numeric SKEDB Version (not the specification Version): Version 1.0 = 1
    pub skedb_version: u32,
    /// Load Step
    pub load_step: Pd3p2SceLoadStep,
    /// Sink Load Characteristics
    pub sink_load_characteristics: Pd3p2SinkLoadCharacteristics,
    /// Compliance
    pub compliance: SCEDCompliance,
    /// Touch Temperature conforms to:
    pub touch_temp: Pd3p2SceTouchTemp,
    /// Battery Info
    pub battery_info: u32,
    /// Sink Modes
    pub sink_modes: SCEDSinkModes,
    /// Sink Minimum PDP
    pub sink_minimum_pdp: u32,
    /// Sink Operational PDP
    pub sink_operational_pdp: u32,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SceVoltageRegulation {
    /// 00b: 150mA/µs Load Step (default)
    pub load_step_150ma: bool,
    /// 01b: 500mA/µs Load Step
    pub load_step_500ma: bool,
    /// 0b: 25% IoC (default)
    pub ioc_25_percent: bool,
    /// 1b: 90% IoC
    pub ioc_90_percent: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SceCompliance {
    /// LPS compliant when set
    pub lps_compliant: bool,
    /// PS1 compliant when set
    pub ps1_compliant: bool,
    /// PS2 compliant when set
    pub ps2_compliant: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SceTouchCurrent {
    /// Low touch Current EPS when set
    pub low_touch_current_eps: bool,
    /// Ground pin supported when set
    pub ground_pin_supported: bool,
    /// Ground pin intended for protective earth when set
    pub ground_pin_for_protective_earth: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2ScePeakCurrent {
    /// Percent overload in 10% increments. Values higher than 25 (11001b)
    /// are clipped to 250%.
    pub percent_overload: bool,
    /// Overload period in 20ms
    pub overload_period: bool,
    /// Duty cycle in 5% increments
    pub duty_cycle: bool,
    /// VBUS Voltage droop
    pub vbus_voltage_droop: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SceSourceInputs {
    /// No external supply when set
    pub no_external_supply: bool,
    /// External supply is constrained when set
    pub external_supply_constrained: bool,
    /// Internal battery is present when set
    pub internal_battery_present: bool,
}

/// This struct represents the Source Capabilities Extended Data.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2SourceCapabilitiesExtended {
    /// Numeric Vendor ID (assigned by the USB-IF)
    pub vid: u32,
    /// Numeric Product ID (assigned by the manufacturer)
    pub pid: u32,
    /// Numeric Value provided by the USB-IF assigned to the product
    pub xid: u32,
    /// Numeric Firmware version number
    pub fw_version: u32,
    /// Numeric Hardware version number
    pub hw_version: u32,
    /// Voltage Regulation
    pub voltage_regulation: Pd3p2SceVoltageRegulation,
    /// Holdup Time
    pub holdup_time: u32,
    /// Compliance
    pub compliance: Pd3p2SceCompliance,
    /// Touch Current
    pub touch_current: Pd3p2SceTouchCurrent,
    /// Peak Current1
    pub peak_current1: Pd3p2ScePeakCurrent,
    /// Peak Current2
    pub peak_current2: Pd3p2ScePeakCurrent,
    /// Peak Current3
    pub peak_current3: Pd3p2ScePeakCurrent,
    /// Touch Temperature conforms to:
    pub touch_temp: Pd3p2SceTouchTemp,
    /// Source Inputs
    pub source_inputs: Pd3p2SceSourceInputs,
    /// Number of Batteries/Battery Slots
    pub num_batteries_slots: u32,
    /// SPR Source PDP Rating
    pub spr_source_pdp_rating: u32,
    /// EPR Source PDP Rating
    pub epr_source_pdp_rating: u32,
}

/// See USPD - 6.5.3 Get_Battery_Cap Message
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2BatteryCapData {
    pub batteries_fixed: [u32; 4],
    pub batteries_hotswappable: [u32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct BSDBatteryInfo {
    /// Invalid Battery reference
    pub invalid_battery_reference: bool,
    /// Battery is present when set
    pub battery_present: bool,
    /// Battery is Charging.
    pub battery_charging: bool,
    /// Battery is Discharging.
    pub battery_discharging: bool,
    /// Battery is Idle.
    pub battery_idle: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2BatteryStatusData {
    /// Battery’s State of Charge (SoC) in 0.1 WH increments
    /// Note: 0xFFFF = Battery’s SOC unknown
    pub battery_present_capacity: u32,
    /// Battery Info
    pub battery_info: BSDBatteryInfo,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2RevisionMessageData {
    /// Revision.major
    pub revision_major: u32,
    /// Revision.minor
    pub revision_minor: u32,
    /// Version.major
    pub version_major: u32,
    /// Version.minor
    pub version_minor: u32,
    /// Reserved, Shall be set to zero
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// See USB PD 3.2 - Table 6.17 “Fixed Supply PDO – Sink”
pub enum Pd3p2FastRoleSwap {
    #[default]
    NotSupported,
    DefaultUsbPower,
    OnePointFiveAAtFiveV,
    ThreeAAtFiveV,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
/// See USB PD 3.2 - Table 6.17 “Fixed Supply PDO – Sink”
pub struct Pd3p2FixedSupplyPdo {
    pub dual_role_power: bool,
    pub higher_capability: bool,
    pub unconstrained_power: bool,
    pub usb_communications_capable: bool,
    pub dual_role_data: bool,
    pub fast_role_swap: Pd3p2FastRoleSwap,
    pub voltage: Millivolt,
    pub operational_current: Milliamp,
}

impl FromBytes for Pd3p2FixedSupplyPdo {
    fn from_bytes(reader: &mut crate::BitReader) -> Result<Self>
    where
        Self: Sized,
    {
        let _ = reader.read::<u32>(2)?; // Fixed supply
        let dual_role_power = reader.read_bit()?;
        let higher_capability = reader.read_bit()?;
        let unconstrained_power = reader.read_bit()?;
        let usb_communications_capable = reader.read_bit()?;
        let dual_role_data = reader.read_bit()?;
        let fast_role_swap_bits = reader.read::<u32>(2)?;
        let fast_role_swap =
            Pd3p2FastRoleSwap::n(fast_role_swap_bits).ok_or_else(|| Error::ParseError {
                field: "fast_role_swap".into(),
                value: fast_role_swap_bits,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        let voltage = (reader.read::<u32>(10)? / 50).into();
        let operational_current = (reader.read::<u32>(10)? / 10).into();

        Ok(Self {
            dual_role_power,
            higher_capability,
            unconstrained_power,
            usb_communications_capable,
            dual_role_data,
            fast_role_swap,
            voltage,
            operational_current,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct Pd3p2BatterySupplyPdo {
    pub max_voltage: Millivolt,
    pub min_voltage: Millivolt,
    pub operational_power: Milliwatt,
}

impl FromBytes for Pd3p2BatterySupplyPdo {
    fn from_bytes(bit_reader: &mut crate::BitReader) -> Result<Self>
    where
        Self: Sized,
    {
        let _ = bit_reader.read::<u32>(2)?; // Battery
        let max_voltage = (bit_reader.read::<u32>(10)? / 50).into();
        let min_voltage = (bit_reader.read::<u32>(10)? / 50).into();
        let operational_power = (bit_reader.read::<u32>(10)? / 10).into();

        Ok(Self {
            max_voltage,
            min_voltage,
            operational_power,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct Pd3p2VariableSupplyPdo {
    pub max_voltage: Millivolt,
    pub min_voltage: Millivolt,
    pub max_current: Milliamp,
}

impl FromBytes for Pd3p2VariableSupplyPdo {
    fn from_bytes(reader: &mut crate::BitReader) -> Result<Self>
    where
        Self: Sized,
    {
        let _ = reader.read::<u32>(2)?; // Variable supply
        let max_voltage = (reader.read::<u32>(10)? / 50).into();
        let min_voltage = (reader.read::<u32>(10)? / 50).into();
        let max_current = (reader.read::<u32>(10)? / 10).into();

        Ok(Self {
            max_voltage,
            min_voltage,
            max_current,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct Pd3p2SprProgrammableSupplyPdo {
    pub max_voltage: Millivolt,
    pub min_voltage: Millivolt,
    pub max_current: Milliamp,
}

impl FromBytes for Pd3p2SprProgrammableSupplyPdo {
    fn from_bytes(reader: &mut crate::BitReader) -> Result<Self>
    where
        Self: Sized,
    {
        let _ = reader.read::<u32>(2)?; // APDO.
        let _ = reader.read::<u32>(2)?; // Programmable power supply
        let _ = reader.read_bit()?; // PPS power limited
        let _reserved1 = reader.read::<u32>(2)?;
        let max_voltage = (reader.read::<u32>(8)? / 50).into();
        let _reserved2 = reader.read_bit()?;
        let min_voltage = (reader.read::<u32>(8)? / 50).into();
        let _reserved3 = reader.read_bit()?;
        let max_current = (reader.read::<u32>(7)? / 10).into();

        Ok(Self {
            max_voltage,
            min_voltage,
            max_current,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub enum PdPdo {
    Pd3p2FixedSupplyPdo(Pd3p2FixedSupplyPdo),
    Pd3p2BatterySupplyPdo(Pd3p2BatterySupplyPdo),
    Pd3p2VariableSupplyPdo(Pd3p2VariableSupplyPdo),
    Pd3p2AugmentedPdo(Pd3p2SprProgrammableSupplyPdo),
}

impl PdPdo {
    pub fn from_bytes(reader: &mut BitReader, revision: BcdWrapper) -> Result<Self> {
        // See USB PD 3.2. - Table 6.7 “Power Data Object”
        let pdo_type = reader.read::<u32>(2)?;
        match pdo_type {
            0 => match revision.0 {
                0x310 => {
                    let pdo = Pd3p2FixedSupplyPdo::from_bytes(reader)?;
                    Ok(PdPdo::Pd3p2FixedSupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            1 => match revision.0 {
                0x310 => {
                    let pdo = Pd3p2BatterySupplyPdo::from_bytes(reader)?;
                    Ok(PdPdo::Pd3p2BatterySupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            2 => match revision.0 {
                0x310 => {
                    let pdo = Pd3p2VariableSupplyPdo::from_bytes(reader)?;
                    Ok(PdPdo::Pd3p2VariableSupplyPdo(pdo))
                }
                _ => Err(Error::UnsupportedUsbRevision {
                    revision,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }),
            },
            3 => match revision.0 {
                0x310 => {
                    let pdo = Pd3p2SprProgrammableSupplyPdo::from_bytes(reader)?;
                    Ok(PdPdo::Pd3p2AugmentedPdo(pdo))
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
