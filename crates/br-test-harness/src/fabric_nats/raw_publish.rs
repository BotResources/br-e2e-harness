use br_core_integration::CommandCoords;
use br_util_nats_fabric::command_subject;

use super::FabricTestNats;

impl FabricTestNats {
    pub async fn publish_command_raw(&self, coords: &CommandCoords, bytes: &[u8]) {
        let subject = command_subject(coords);
        self.client
            .publish(subject, bytes.to_vec().into())
            .await
            .expect("publish raw command bytes onto the fabric");
        self.client
            .flush()
            .await
            .expect("flush raw command bytes onto the fabric");
    }
}
