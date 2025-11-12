// crates/flowcore/src/events/iggy_bus.rs

// crates/flowcore/src/events/iggy_bus.rs

use iggy::clients::client::IggyClient;
use iggy::prelude::*;
use serde_json;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;
use futures_util::StreamExt;

use crate::{EventEmitter, EventBus, ExecutionEvent, NodeEvent};

/// Configuration for Iggy event bus
#[derive(Debug, Clone)]
pub struct IggyEventBusConfig {
    pub connection_string: String,
    pub stream_name: String,
    pub topic_name: String,
    pub username: String,
    pub password: String,
}

impl Default for IggyEventBusConfig {
    fn default() -> Self {
        Self {
            connection_string: "iggy://iggy:iggy@127.0.0.1:8090".to_string(),
            stream_name: "flowengine".to_string(),
            topic_name: "workflow_events".to_string(),
            username: "iggy".to_string(),
            password: "iggy".to_string(),
        }
    }
}

/// Event bus backed by Apache Iggy 0.7
pub struct IggyEventBus {
    client: Arc<IggyClient>,
    config: IggyEventBusConfig,
    stream_id: u32,
    topic_id: u32,
}

impl IggyEventBus {
    /// Create a new Iggy event bus
    pub async fn new(config: IggyEventBusConfig) -> Result<Self, IggyEventBusError> {
        tracing::info!("Connecting to Iggy server: {}", config.connection_string);
        
        // Create client from connection string
        let client = IggyClient::from_connection_string(&config.connection_string)
            .map_err(|e| {
                tracing::error!("Failed to create client from connection string: {:?}", e);
                IggyEventBusError::ConnectionFailed(format!("Client creation failed: {}", e))
            })?;
        
        tracing::debug!("Client created, attempting to connect...");
        
        // Connect to the server
        client
            .connect()
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to server: {:?}", e);
                IggyEventBusError::ConnectionFailed(format!("Connection failed: {}", e))
            })?;
        
        tracing::info!("Connected to Iggy server successfully");
        
        // Note: Authentication might already be handled by the connection string
        // Only call login_user if needed
        tracing::debug!("Attempting explicit authentication...");
        match client.login_user(&config.username, &config.password).await {
            Ok(_) => tracing::info!("Authenticated successfully"),
            Err(e) => {
                tracing::warn!("Explicit authentication returned error (might already be authenticated): {:?}", e);
                // Don't fail here - connection string auth might have already worked
            }
        }
        
        let mut bus = Self {
            client: Arc::new(client),
            config: config.clone(),
            stream_id: 0,
            topic_id: 0,
        };
        
        // Ensure stream and topic exist
        bus.ensure_stream_and_topic().await?;
        
        Ok(bus)
    }
    
    /// Ensure stream and topic exist
    async fn ensure_stream_and_topic(&mut self) -> Result<(), IggyEventBusError> {
        tracing::debug!("Creating stream: {}", self.config.stream_name);
        
        // Try to create stream
        let stream_details = match self.client.create_stream(&self.config.stream_name, None).await {
            Ok(details) => {
                tracing::info!("Created stream: {} with ID: {}", self.config.stream_name, details.id);
                details
            }
            Err(e) => {
                tracing::debug!("Stream creation failed (might already exist): {:?}", e);
                // Try to get existing stream
                let stream_identifier: Identifier = self.config.stream_name.as_str().try_into()
                    .map_err(|e| {
                        tracing::error!("Invalid stream name '{}': {:?}", self.config.stream_name, e);
                        IggyEventBusError::ConnectionFailed(format!("Invalid stream name: {}", e))
                    })?;
                
                self.client.get_stream(&stream_identifier).await
                    .map_err(|e| {
                        tracing::error!("Failed to get stream: {:?}", e);
                        IggyEventBusError::ConnectionFailed(format!("Failed to get stream: {}", e))
                    })?
                    .ok_or_else(|| IggyEventBusError::ConnectionFailed("Stream not found".to_string()))?
            }
        };
        
        self.stream_id = stream_details.id;
        tracing::info!("Using stream ID: {}", self.stream_id);
        
        // Try to create topic
        let stream_id_identifier: Identifier = self.stream_id.try_into()
            .map_err(|e| {
                tracing::error!("Invalid stream ID {}: {:?}", self.stream_id, e);
                IggyEventBusError::ConnectionFailed(format!("Invalid stream ID: {}", e))
            })?;
        
        tracing::debug!("Creating topic: {} in stream ID: {}", self.config.topic_name, self.stream_id);
        
        let topic_details = match self.client.create_topic(
            &stream_id_identifier,
            &self.config.topic_name,
            1, // partitions
            CompressionAlgorithm::default(),
            None, // replication factor
            None, // topic_id (let server assign)
            IggyExpiry::NeverExpire,
            MaxTopicSize::ServerDefault,
        ).await {
            Ok(details) => {
                tracing::info!("Created topic: {} with ID: {}", self.config.topic_name, details.id);
                details
            }
            Err(e) => {
                tracing::debug!("Topic creation failed (might already exist): {:?}", e);
                // Try to get existing topic
                let topic_identifier: Identifier = self.config.topic_name.as_str().try_into()
                    .map_err(|e| {
                        tracing::error!("Invalid topic name '{}': {:?}", self.config.topic_name, e);
                        IggyEventBusError::ConnectionFailed(format!("Invalid topic name: {}", e))
                    })?;
                
                self.client.get_topic(&stream_id_identifier, &topic_identifier).await
                    .map_err(|e| {
                        tracing::error!("Failed to get topic: {:?}", e);
                        IggyEventBusError::ConnectionFailed(format!("Failed to get topic: {}", e))
                    })?
                    .ok_or_else(|| IggyEventBusError::ConnectionFailed("Topic not found".to_string()))?
            }
        };
        
        self.topic_id = topic_details.id;
        tracing::info!("Using topic ID: {} (partitions: {})", self.topic_id, topic_details.partitions_count);
        
        Ok(())
    }
    
    /// Publish an event to the bus using low-level client API
    pub async fn publish(&self, event: ExecutionEvent) -> Result<(), IggyEventBusError> {
        let payload = serde_json::to_vec(&event)
            .map_err(|e| {
                tracing::error!("Failed to serialize event: {:?}", e);
                IggyEventBusError::SerializationFailed(e.to_string())
            })?;
        
        tracing::debug!(
            "Publishing message to stream ID: {}, topic ID: {}, payload size: {} bytes",
            self.stream_id,
            self.topic_id,
            payload.len()
        );
        
        // Use numeric IDs
        let stream_id: Identifier = self.stream_id.try_into()
            .map_err(|e| {
                tracing::error!("Invalid stream ID {}: {:?}", self.stream_id, e);
                IggyEventBusError::PublishFailed(format!("Invalid stream ID: {}", e))
            })?;
        
        let topic_id: Identifier = self.topic_id.try_into()
            .map_err(|e| {
                tracing::error!("Invalid topic ID {}: {:?}", self.topic_id, e);
                IggyEventBusError::PublishFailed(format!("Invalid topic ID: {}", e))
            })?;
        
        // Create message from payload
        let message = IggyMessage::from(payload);
        let mut messages = vec![message];
        
        tracing::debug!("Created {} message(s), preparing to send", messages.len());
        
        // IMPORTANT: Use balanced partitioning or specify partition 0 (partitions are 0-indexed!)
        // Try balanced first, which should work with any partition count
        let partitioning = Partitioning::balanced();
        
        tracing::debug!("Using partitioning strategy: balanced");
        
        // Send message using low-level API
        match self.client
            .send_messages(&stream_id, &topic_id, &partitioning, &mut messages)
            .await
        {
            Ok(_) => {
                tracing::debug!("Message sent successfully");
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "Failed to send message to stream {}, topic {}: {:?}",
                    self.stream_id,
                    self.topic_id,
                    e
                );
                
                // Try to provide more context
                let error_msg = format!(
                    "Send failed: {:?} (stream_id: {}, topic_id: {}, partitioning: balanced)",
                    e, self.stream_id, self.topic_id
                );
                
                Err(IggyEventBusError::PublishFailed(error_msg))
            }
        }
    }
    
    /// Subscribe to events from the bus using high-level consumer
    pub async fn subscribe(
        &self,
        consumer_id: String,
    ) -> Result<IggyEventSubscription, IggyEventBusError> {
        tracing::info!("Creating subscription with consumer_id: {}", consumer_id);
        Ok(IggyEventSubscription {
            client: self.client.clone(),
            stream_name: self.config.stream_name.clone(),
            topic_name: self.config.topic_name.clone(),
            consumer_id,
        })
    }
}

/// Subscription handle for consuming events
pub struct IggyEventSubscription {
    client: Arc<IggyClient>,
    stream_name: String,
    topic_name: String,
    consumer_id: String,
}

impl IggyEventSubscription {
    /// Poll for new events using high-level consumer
    pub async fn poll(&self) -> Result<Vec<ExecutionEvent>, IggyEventBusError> {
        tracing::debug!("Polling for events from consumer group: {}", self.consumer_id);
        
        // Create consumer using consumer_group
        let mut consumer = self.client
            .consumer_group(&self.consumer_id, &self.stream_name, &self.topic_name)
            .map_err(|e| {
                tracing::error!("Failed to create consumer group: {:?}", e);
                IggyEventBusError::PollFailed(format!("Consumer group creation failed: {}", e))
            })?
            .auto_join_consumer_group()
            .create_consumer_group_if_not_exists()
            .polling_strategy(PollingStrategy::next())
            .build();
        
        tracing::debug!("Consumer created, initializing...");
        
        // Initialize consumer
        consumer
            .init()
            .await
            .map_err(|e| {
                tracing::error!("Failed to initialize consumer: {:?}", e);
                IggyEventBusError::PollFailed(format!("Consumer initialization failed: {}", e))
            })?;
        
        tracing::debug!("Consumer initialized, polling for messages...");
            
        // Receive messages using next()
        let mut events = Vec::new();
        while let Some(result) = consumer.next().await {
            match result {
                Ok(received_message) => {
                    tracing::debug!("Received message with {} bytes", received_message.message.payload.len());
                    match serde_json::from_slice::<ExecutionEvent>(&received_message.message.payload) {
                        Ok(event) => {
                            tracing::debug!("Successfully deserialized event");
                            events.push(event);
                        }
                        Err(e) => {
                            tracing::error!("Failed to deserialize event: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to receive message: {:?}", e);
                }
            }
            // Only get one batch for now
            break;
        }
        
        tracing::info!("Polled {} events from consumer group: {}", events.len(), self.consumer_id);
        
        Ok(events)
    }
}

#[derive(Debug)]
pub enum IggyEventBusError {
    ConnectionFailed(String),
    SerializationFailed(String),
    PublishFailed(String),
    PollFailed(String),
    NotFound,
}

impl fmt::Display for IggyEventBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Failed to connect to Iggy: {}", msg),
            Self::SerializationFailed(msg) => write!(f, "Failed to serialize event: {}", msg),
            Self::PublishFailed(msg) => write!(f, "Failed to publish event: {}", msg),
            Self::PollFailed(msg) => write!(f, "Failed to poll events: {}", msg),
            Self::NotFound => write!(f, "Stream or topic not found"),
        }
    }
}

impl StdError for IggyEventBusError {}