//! Makes `EditPermission` requests regarding `ExternalCanister` to Orbit.
use clap::{Parser, Subcommand};

use crate::{args::request::CreateRequestArgs, orbit_station_agent::StationAgent};

/// Request canister changes.
#[derive(Debug, Subcommand)]
#[command(version, about, long_about = None)]
pub enum Args {
    /// Request changes to canister permissions.
    Change(ChangeCanister),
}

impl CreateRequestArgs for Args {
    /// Converts the CLI arg type into the equivalent Orbit API type.
    fn into_create_request_input(
        self,
        station_agent: &StationAgent,
    ) -> anyhow::Result<orbit_station_api::CreateRequestInput> {
        match self {
            Args::Change(change_args) => change_args.into_create_request_input(station_agent),
        }
    }
}

/// Requests the privilige of proposing canister upgrades.
#[derive(Debug, Parser)]
pub struct ChangeCanister {
    /// Canister name or ID.
    // TODO: If a canister is not specified, require --all.
    #[structopt(long)]
    pub canister: Option<String>,
}

impl CreateRequestArgs for ChangeCanister {
    /// Converts the CLI arg type into the equivalent Orbit API type.
    fn into_create_request_input(
        self,
        station_agent: &StationAgent,
    ) -> anyhow::Result<orbit_station_api::CreateRequestInput> {
        let canisters: anyhow::Result<orbit_station_api::ChangeExternalCanisterResourceTargetDTO> =
            if let Some(canister_name_or_id) = self.canister {
                station_agent
                    .canister_id(&canister_name_or_id)
                    .map(orbit_station_api::ChangeExternalCanisterResourceTargetDTO::Canister)
            } else {
                Ok(orbit_station_api::ChangeExternalCanisterResourceTargetDTO::Any)
            };

        let resource = orbit_station_api::ResourceDTO::ExternalCanister(
            orbit_station_api::ExternalCanisterResourceActionDTO::Change(canisters?),
        );

        let operation = orbit_station_api::RequestOperationInput::EditPermission(
            orbit_station_api::EditPermissionOperationInput {
                resource,
                auth_scope: None,
                users: None,
                user_groups: None,
            },
        );
        Ok(orbit_station_api::CreateRequestInput {
            operation,
            title: None,
            summary: None,
            execution_plan: None,
        })
    }
}
