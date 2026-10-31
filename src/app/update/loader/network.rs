use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, NetworkMsg, WindowMsg};

impl FluxApp {
    /// Asynchronously lists a network location via GVFS and dispatches the result.
    pub fn load_network(
        &mut self,
        uri: &str,
        credentials: Option<crate::domain::model::NetworkCredentials>,
        sender: relm4::AsyncComponentSender<Self>,
    ) {
        crate::hit!("load_network");
        let uri_str = uri.to_string();
        let expand_labels = self.config.ui.expand_labels;

        relm4::spawn_local(async move {
            match crate::infra::network::list_network_contexts(
                &uri_str,
                credentials.as_ref(),
                expand_labels,
            )
            .await
            {
                Ok(contexts) => {
                    sender.input(AppMsg::Network(NetworkMsg::NetworkLoaded {
                        uri: uri_str,
                        contexts,
                    }));
                }
                Err(crate::infra::network::NetworkError::CredentialsRequired {
                    message,
                    flags,
                }) => {
                    sender.input(AppMsg::Network(NetworkMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message,
                        flags,
                        auth_failed: false,
                    }));
                }
                Err(crate::infra::network::NetworkError::AuthFailed) => {
                    sender.input(AppMsg::Network(NetworkMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message: crate::i18n::tr(
                            "Authentication failed. Please check your credentials.",
                        ),
                        flags: crate::domain::model::NetworkAuthFlags::USERNAME
                            | crate::domain::model::NetworkAuthFlags::PASSWORD,
                        auth_failed: true,
                    }));
                }
                Err(e) => {
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(e.to_string())));
                }
            }
        });
    }
}
