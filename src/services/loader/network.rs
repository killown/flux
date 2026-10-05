use crate::model::{AppMsg, FluxApp};

impl FluxApp {
    /// Asynchronously lists a network location via GVFS and dispatches the result.
    pub fn load_network(
        &mut self,
        uri: &str,
        credentials: Option<crate::services::network::NetworkCredentials>,
        sender: relm4::AsyncComponentSender<Self>,
    ) {
        let uri_str = uri.to_string();
        let expand_labels = self.config.ui.expand_labels;

        relm4::spawn_local(async move {
            match crate::services::network::list_network_entries(&uri_str, credentials.as_ref())
                .await
            {
                Ok(entries) => {
                    let contexts =
                        crate::services::network::entries_to_load_contexts(&entries, expand_labels);
                    sender.input(AppMsg::NetworkLoaded {
                        uri: uri_str,
                        contexts,
                    });
                }
                Err(crate::services::network::NetworkError::CredentialsRequired {
                    message,
                    flags,
                }) => {
                    sender.input(AppMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message,
                        flags,
                        auth_failed: false,
                    });
                }
                Err(crate::services::network::NetworkError::AuthFailed) => {
                    sender.input(AppMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message: crate::i18n::tr(
                            "Authentication failed. Please check your credentials.",
                        ),
                        flags: crate::services::network::NetworkAuthFlags::USERNAME
                            | crate::services::network::NetworkAuthFlags::PASSWORD,
                        auth_failed: true,
                    });
                }
                Err(e) => {
                    sender.input(AppMsg::ShowToast(e.to_string()));
                }
            }
        });
    }
}
