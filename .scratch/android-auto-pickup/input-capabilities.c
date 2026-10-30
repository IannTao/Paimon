/* Capability probe only: no device creation, event writes, or exclusive grab. */
#include <errno.h>
#include <fcntl.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include <unistd.h>

int main(void) {
    errno = 0;
    int ev_rw = open("/dev/input/event6", O_RDWR | O_CLOEXEC | O_NONBLOCK);
    int ev_rw_errno = ev_rw < 0 ? errno : 0;
    if (ev_rw >= 0) close(ev_rw);

    errno = 0;
    int ev = open("/dev/input/event6", O_RDONLY | O_CLOEXEC | O_NONBLOCK);
    int ev_errno = ev < 0 ? errno : 0;
    int ev_version = -1, ev_version_errno = 0;
    int release_result = -1, release_errno = 0;
    if (ev >= 0) {
        if (ioctl(ev, EVIOCGVERSION, &ev_version) < 0) ev_version_errno = errno;
        errno = 0;
        /* Fresh fd has never grabbed anything. arg=0 only tries to release
           this fd's nonexistent grab; EINVAL means the ioctl reached evdev. */
        release_result = ioctl(ev, EVIOCGRAB, 0);
        release_errno = release_result < 0 ? errno : 0;
        close(ev);
    }

    errno = 0;
    int ui = open("/dev/uinput", O_RDWR | O_CLOEXEC | O_NONBLOCK);
    int ui_errno = ui < 0 ? errno : 0;
    int ui_version = -1, ui_version_errno = 0;
    if (ui >= 0) {
        if (ioctl(ui, UI_GET_VERSION, &ui_version) < 0) ui_version_errno = errno;
        close(ui);
    }
    printf("{\n  \"uid\": %d,\n  \"evdevReadWriteErrno\": %d,\n"
           "  \"evdevReadOnlyErrno\": %d,\n  \"evdevVersion\": %d,\n"
           "  \"evdevVersionErrno\": %d,\n  \"unownedGrabReleaseResult\": %d,\n"
           "  \"unownedGrabReleaseErrno\": %d,\n  \"uinputOpenErrno\": %d,\n"
           "  \"uinputVersion\": %d,\n  \"uinputVersionErrno\": %d,\n"
           "  \"createdDevice\": false,\n  \"grabbedDevice\": false,\n"
           "  \"wroteInputEvents\": false\n}\n",
           getuid(), ev_rw_errno, ev_errno, ev_version, ev_version_errno,
           release_result, release_errno, ui_errno, ui_version, ui_version_errno);
    return 0;
}
