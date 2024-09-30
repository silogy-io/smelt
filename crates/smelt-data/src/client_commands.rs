use self::client_command::ClientCommands;

tonic::include_proto!("smelt_client.commands");

impl ClientCommand {
    pub fn send_graph(graph_string: String, def_path: String) -> Self {
        let cc = ClientCommands::Setter(SetCommands {
            command_content: graph_string,
            maybe_def_path: def_path,
        });

        ClientCommand {
            client_commands: Some(cc),
        }
    }

    pub fn execute_command(command_name: String) -> Self {
        let cc = ClientCommands::Runone(RunOne { command_name });

        ClientCommand {
            client_commands: Some(cc),
        }
    }

    pub fn execute_many(command_names: Vec<String>) -> Self {
        let cc = ClientCommands::Runmany(RunMany { command_names });

        ClientCommand {
            client_commands: Some(cc),
        }
    }

    pub fn execute_type(typeinfo: String) -> Self {
        let cc = ClientCommands::Runtype(RunType { typeinfo });

        ClientCommand {
            client_commands: Some(cc),
        }
    }

    pub fn get_cfg() -> Self {
        let cc = ClientCommands::Getcfg(GetConfig {});

        ClientCommand {
            client_commands: Some(cc),
        }
    }
}
