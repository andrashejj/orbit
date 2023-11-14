use super::UserService;
use crate::{
    core::{generate_uuid_v4, ic_cdk::api::time, CallContext, WithCallContext},
    errors::NotificationError,
    mappers::HelperMapper,
    models::{Notification, NotificationId, NotificationStatus, NotificationType, UserId},
    repositories::{NotificationFindByUserWhereClause, NotificationRepository},
    transport::{ListNotificationsInput, MarkNotificationsReadInput},
};
use ic_canister_core::repository::Repository;
use ic_canister_core::utils::rfc3339_to_timestamp;
use ic_canister_core::{api::ServiceResult, model::ModelValidator};
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct NotificationService {
    call_context: CallContext,
    user_service: UserService,
    notification_repository: NotificationRepository,
}

impl WithCallContext for NotificationService {
    fn with_call_context(call_context: CallContext) -> Self {
        Self {
            call_context: call_context.clone(),
            user_service: UserService::with_call_context(call_context.clone()),
            ..Default::default()
        }
    }
}

impl NotificationService {
    pub fn get_notification(&self, id: &NotificationId) -> ServiceResult<Notification> {
        let notification = self
            .notification_repository
            .get(&Notification::key(*id))
            .ok_or(NotificationError::NotFound {
                id: Uuid::from_bytes(id.to_owned()).hyphenated().to_string(),
            })?;

        self.assert_notification_access(&notification)?;

        Ok(notification)
    }

    pub fn list_notifications(
        &self,
        input: ListNotificationsInput,
    ) -> ServiceResult<Vec<Notification>> {
        let user = self
            .user_service
            .get_user_by_identity(&self.call_context.caller())?;

        let filter_by_type = input.notification_type.map(|t| t.to_string());

        let notifications = self.notification_repository.find_by_user_where(
            user.id,
            NotificationFindByUserWhereClause {
                created_dt_from: input.from_dt.map(|dt| rfc3339_to_timestamp(dt.as_str())),
                created_dt_to: input.to_dt.map(|dt| rfc3339_to_timestamp(dt.as_str())),
                notification_type: filter_by_type,
                status: input.status.map(|status| status.into()),
            },
        );

        Ok(notifications)
    }

    pub async fn mark_read(&self, input: MarkNotificationsReadInput) -> ServiceResult<()> {
        let mut notifications = input
            .notification_ids
            .iter()
            .map(|id| self.get_notification(HelperMapper::to_uuid(id.clone())?.as_bytes()))
            .collect::<Result<Vec<Notification>, _>>()?;

        for notification in notifications.iter_mut() {
            notification.status = match input.read {
                true => NotificationStatus::Read,
                false => NotificationStatus::Sent,
            };
            notification.last_modification_timestamp = time();

            notification.validate()?;
            self.notification_repository
                .insert(notification.to_key(), notification.clone());
        }

        Ok(())
    }

    pub async fn send_notification(
        &self,
        user_id: UserId,
        notification_type: NotificationType,
        title: Option<(String, String)>,
        message: Option<(String, String)>,
    ) -> ServiceResult<()> {
        let notification_id = generate_uuid_v4().await;
        let notification = Notification {
            id: *notification_id.as_bytes(),
            status: NotificationStatus::Sent,
            target_user_id: user_id,
            title: match title {
                Some(title) => title,
                None => match &notification_type {
                    NotificationType::SystemMessage => (
                        "system_message_title".to_string(),
                        "system_message_title".to_string(),
                    ),
                    NotificationType::ProposalCreated(_) => (
                        "New proposal created".to_string(),
                        "notification_proposal_created".to_string(),
                    ),
                    NotificationType::TransferProposalCreated(_) => (
                        "New transfer requested".to_string(),
                        "notification_transfer_proposal_created_title".to_string(),
                    ),
                },
            },
            message: match message {
                Some(message) => message,
                None => match &notification_type {
                    NotificationType::SystemMessage => ("".to_string(), "".to_string()),
                    NotificationType::ProposalCreated(_) => (
                        "Please review it and vote on the action to be taken.".to_string(),
                        "notification_proposal_created_message".to_string(),
                    ),
                    NotificationType::TransferProposalCreated(_) => (
                        "Please review it and vote on the action to be taken.".to_string(),
                        "notification_transfer_proposal_created_message".to_string(),
                    ),
                },
            },
            notification_type,
            created_timestamp: time(),
            last_modification_timestamp: time(),
        };

        notification.validate()?;

        self.notification_repository
            .insert(notification.to_key(), notification);

        Ok(())
    }

    fn assert_notification_access(&self, notification: &Notification) -> ServiceResult<()> {
        let user = self
            .user_service
            .get_user_by_identity(&self.call_context.caller())?;

        if user.id != notification.target_user_id {
            Err(NotificationError::Forbidden {
                id: Uuid::from_bytes(notification.id.to_owned())
                    .hyphenated()
                    .to_string(),
            })?
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::test_utils,
        models::{notification_test_utils::mock_notification, user_test_utils::mock_user, User},
        repositories::UserRepository,
    };
    use candid::Principal;

    struct TestContext {
        repository: NotificationRepository,
        service: NotificationService,
        caller_user: User,
    }

    fn setup() -> TestContext {
        test_utils::init_canister_config();

        let call_context = CallContext::new(Principal::from_slice(&[9; 29]));
        let mut user = mock_user();
        user.identities = vec![call_context.caller()];

        UserRepository::default().insert(user.to_key(), user.clone());

        TestContext {
            repository: NotificationRepository::default(),
            service: NotificationService::with_call_context(call_context),
            caller_user: user,
        }
    }

    #[test]
    fn get_notification() {
        let ctx = setup();
        let mut notification = mock_notification();
        notification.target_user_id = ctx.caller_user.id;

        ctx.repository
            .insert(notification.to_key(), notification.to_owned());

        let result = ctx.service.get_notification(&notification.id);

        assert_eq!(notification, result.unwrap());
    }

    #[test]
    fn fail_get_notification_not_allowed() {
        let ctx = setup();
        let mut notification = mock_notification();
        notification.target_user_id = [1; 16];

        ctx.repository
            .insert(notification.to_key(), notification.to_owned());

        let result = ctx.service.get_notification(&notification.id);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_notification_happy_path() {
        let ctx = setup();
        let notification_id = Uuid::new_v4();
        let mut notification = mock_notification();
        notification.id = *notification_id.as_bytes();
        notification.target_user_id = ctx.caller_user.id;
        notification.status = NotificationStatus::Sent;

        ctx.repository
            .insert(notification.to_key(), notification.to_owned());

        let result = ctx
            .service
            .mark_read(MarkNotificationsReadInput {
                notification_ids: vec![notification_id.to_string()],
                read: true,
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(
            ctx.repository.get(&notification.to_key()).unwrap().status,
            NotificationStatus::Read
        );
    }
}