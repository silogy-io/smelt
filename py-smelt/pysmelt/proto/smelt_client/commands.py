# Generated by the protocol buffer compiler.  DO NOT EDIT!
# sources: client.data.proto
# plugin: python-betterproto
from dataclasses import dataclass
from typing import Dict, List

import betterproto


class ProfilingSelection(betterproto.Enum):
    DISABLED = 0
    # only memory and cpu
    SIMPLE_PROF = 1


@dataclass
class ClientCommand(betterproto.Message):
    setter: "SetCommands" = betterproto.message_field(1, group="ClientCommands")
    runone: "RunOne" = betterproto.message_field(2, group="ClientCommands")
    runtype: "RunType" = betterproto.message_field(3, group="ClientCommands")
    runmany: "RunMany" = betterproto.message_field(4, group="ClientCommands")
    getcfg: "GetConfig" = betterproto.message_field(5, group="ClientCommands")


@dataclass
class SetCommands(betterproto.Message):
    command_content: str = betterproto.string_field(1)


@dataclass
class RunOne(betterproto.Message):
    command_name: str = betterproto.string_field(1)


@dataclass
class RunMany(betterproto.Message):
    command_names: List[str] = betterproto.string_field(1)


@dataclass
class RunType(betterproto.Message):
    # Eventually, perhaps we should encode this as info in protobuf not today
    # babey
    typeinfo: str = betterproto.string_field(1)


@dataclass
class GetConfig(betterproto.Message):
    pass


@dataclass
class ClientResp(betterproto.Message):
    """Responses to the client command"""

    current_cfg: "ConfigureSmelt" = betterproto.message_field(
        1, group="ClientResponses"
    )


@dataclass
class ConfigureSmelt(betterproto.Message):
    """
    This configuration is done once, when SMELT is initialized The client
    should provide this when creating an smelt handle
    """

    # Should be an absolute path
    smelt_root: str = betterproto.string_field(1)
    # number of slots the entire executor has -- analogous to job slots in make
    job_slots: int = betterproto.uint64_field(2)
    # configures how we profile commands
    prof_cfg: "ProfilerCfg" = betterproto.message_field(3)
    # If true, we ignore the non test commands
    test_only: bool = betterproto.bool_field(4)
    # If true, we do not transmit stdout from the server
    silent: bool = betterproto.bool_field(5)
    local: "CfgLocal" = betterproto.message_field(10, group="InitExecutor")
    docker: "CfgDocker" = betterproto.message_field(11, group="InitExecutor")


@dataclass
class ProfilerCfg(betterproto.Message):
    # if we enable simple profiling
    prof_type: "ProfilingSelection" = betterproto.enum_field(1)
    sampling_period: int = betterproto.uint64_field(2)


@dataclass
class CfgLocal(betterproto.Message):
    pass


@dataclass
class CfgDocker(betterproto.Message):
    image_name: str = betterproto.string_field(1)
    additional_mounts: Dict[str, str] = betterproto.map_field(
        2, betterproto.TYPE_STRING, betterproto.TYPE_STRING
    )
