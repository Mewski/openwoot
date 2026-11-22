/*
 * app: the work of one pass of the main loop: the two task bodies (USB active / USB
 * suspended) and the services of the host protocol. main calls them; no module does.
 *
 * Depends on: nothing.
 */
#ifndef APP_TASKS_H_
#define APP_TASKS_H_

void startup_run_suspended_task(void);      /* main-loop body while USB is suspended */
void startup_run_active_task(void);         /* main-loop body in normal operation */
void protocol_notify_service(void);         /* pending host notifications */
void protocol_queue_service(void);          /* queued host commands */

#endif /* APP_TASKS_H_ */
