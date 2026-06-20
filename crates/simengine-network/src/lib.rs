use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: Vec<u8>,
}

pub trait NetworkInterface {
    fn publish(&mut self, topic: &str, payload: &[u8]);
    fn drain(&mut self) -> Vec<Message>;
}

#[derive(Default)]
pub struct InProcessNetwork {
    messages: VecDeque<Message>,
}

impl NetworkInterface for InProcessNetwork {
    fn publish(&mut self, topic: &str, payload: &[u8]) {
        self.messages.push_back(Message {
            topic: topic.to_string(),
            payload: payload.to_vec(),
        });
    }

    fn drain(&mut self) -> Vec<Message> {
        self.messages.drain(..).collect()
    }
}
