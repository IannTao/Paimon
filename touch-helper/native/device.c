/* Linux UAPI stays in C so Rust never duplicates ioctl numbers or timeval ABI. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <unistd.h>

struct event { uint16_t kind, code; int32_t value; };
struct caps {
    char name[80];
    int32_t slots, max_x, max_y, max_id, selected, tool_finger;
    int32_t axes[ABS_CNT]; /* INT_MIN: absent; otherwise synthetic initial value */
};
struct device {
    int physical, virtual, created, grabbed;
    unsigned char keys[(KEY_CNT + 7) / 8];
    unsigned char properties[(INPUT_PROP_CNT + 7) / 8];
    unsigned char abs_bits[(ABS_CNT + 7) / 8];
    unsigned char msc_bits[(MSC_CNT + 7) / 8];
    struct input_absinfo axes[ABS_CNT];
};
static volatile sig_atomic_t stopping;
static void stop_signal(int sig) { (void)sig; stopping = 1; }
static int bit(const unsigned char *bits, unsigned int code) { return !!(bits[code / 8] & (1u << (code % 8))); }
static int failure(const char *operation) {
    int error = errno ? errno : EIO;
    fprintf(stderr, "{\"event\":\"syscall_error\",\"operation\":\"%s\",\"errno\":%d}\n", operation, error);
    return -error;
}
#define CHECK(call) do { if ((call) < 0) return failure(#call); } while (0)

int ph_init(int supervisor) {
    if (getuid() != 2000) { errno = EPERM; return failure("require_shell_uid_2000"); }
    CHECK(prctl(PR_SET_PDEATHSIG, SIGKILL));
    /* Check after prctl to close the parent-death race. */
    if (supervisor <= 1 || getppid() != supervisor) { errno = ECHILD; return failure("supervisor_parent"); }
    struct sigaction action = { .sa_handler = stop_signal };
    sigemptyset(&action.sa_mask);
    CHECK(sigaction(SIGTERM, &action, NULL));
    CHECK(sigaction(SIGINT, &action, NULL));
    int flags = fcntl(STDIN_FILENO, F_GETFL);
    CHECK(flags);
    CHECK(fcntl(STDIN_FILENO, F_SETFL, flags | O_NONBLOCK));
    return 0;
}

int ph_close(struct device *d) {
    if (!d) return 0;
    int status = 0;
    if (d->virtual >= 0) {
        if (d->created) {
            if (ioctl(d->virtual, UI_DEV_DESTROY) < 0) status = failure("UI_DEV_DESTROY");
            else { printf("{\"event\":\"destroyed\"}\n"); fflush(stdout); }
        }
        close(d->virtual); /* Closing also destroys on abnormal cleanup. */
    }
    if (d->physical >= 0) {
        if (d->grabbed && ioctl(d->physical, EVIOCGRAB, 0) < 0) status = failure("EVIOCGRAB_release");
        close(d->physical);
    }
    free(d);
    return status;
}

static int read_caps(struct device *d, struct caps *c) {
    unsigned char events[(EV_CNT + 7) / 8] = {0};
    CHECK(ioctl(d->physical, EVIOCGNAME(sizeof(c->name)), c->name));
    c->name[sizeof(c->name) - 1] = 0;
    if (!strncmp(c->name, "Paimon proxy", 12)) { errno = EINVAL; return failure("refuse_proxy_source"); }
    CHECK(ioctl(d->physical, EVIOCGBIT(0, sizeof(events)), events));
    /* Unsupported event families are refused instead of silently discarded. */
    for (int i = 0; i < EV_CNT; ++i) {
        if (bit(events, i) && i != EV_SYN && i != EV_KEY && i != EV_ABS && i != EV_MSC) {
            errno = ENOTSUP; return failure("unsupported_event_family");
        }
    }
    CHECK(ioctl(d->physical, EVIOCGPROP(sizeof(d->properties)), d->properties));
    CHECK(ioctl(d->physical, EVIOCGBIT(EV_KEY, sizeof(d->keys)), d->keys));
    CHECK(ioctl(d->physical, EVIOCGBIT(EV_ABS, sizeof(d->abs_bits)), d->abs_bits));
    CHECK(ioctl(d->physical, EVIOCGBIT(EV_MSC, sizeof(d->msc_bits)), d->msc_bits));
    if (!bit(d->properties, INPUT_PROP_DIRECT) || bit(d->properties, INPUT_PROP_SEMI_MT) ||
        !bit(d->keys, BTN_TOUCH) || !bit(d->abs_bits, ABS_MT_SLOT) ||
        !bit(d->abs_bits, ABS_MT_TRACKING_ID) || !bit(d->abs_bits, ABS_MT_POSITION_X) ||
        !bit(d->abs_bits, ABS_MT_POSITION_Y)) {
        errno = ENOTSUP; return failure("require_direct_type_b_touchscreen");
    }
    /* Contact-count tool keys would require additional merged count semantics. */
    for (int key = BTN_TOOL_PEN; key <= BTN_TOOL_QUADTAP; ++key) {
        if (key != BTN_TOUCH && key != BTN_TOOL_FINGER && bit(d->keys, key)) { errno = ENOTSUP; return failure("unsupported_tool_count_keys"); }
    }
    c->tool_finger = bit(d->keys, BTN_TOOL_FINGER);
    for (int i = 0; i < ABS_CNT; ++i) {
        c->axes[i] = INT_MIN;
        if (!bit(d->abs_bits, i)) continue;
        CHECK(ioctl(d->physical, EVIOCGABS(i), &d->axes[i]));
        if (d->axes[i].minimum == INT_MIN || d->axes[i].maximum < d->axes[i].minimum) {
            errno = ENOTSUP; return failure("unsupported_axis_range");
        }
        c->axes[i] = d->axes[i].minimum > 0 ? d->axes[i].minimum : (d->axes[i].maximum < 0 ? d->axes[i].maximum : 0);
        if (i == ABS_MT_PRESSURE || i == ABS_MT_TOUCH_MAJOR || i == ABS_MT_WIDTH_MAJOR) {
            if (d->axes[i].maximum > c->axes[i]) ++c->axes[i];
        }
    }
    if (d->axes[ABS_MT_SLOT].maximum == INT_MAX) { errno = ENOTSUP; return failure("invalid_slot_maximum"); }
    c->slots = d->axes[ABS_MT_SLOT].maximum + 1;
    c->selected = d->axes[ABS_MT_SLOT].value;
    c->max_x = d->axes[ABS_MT_POSITION_X].maximum;
    c->max_y = d->axes[ABS_MT_POSITION_Y].maximum;
    c->max_id = d->axes[ABS_MT_TRACKING_ID].maximum;
    if (d->axes[ABS_MT_SLOT].minimum != 0 || d->axes[ABS_MT_POSITION_X].minimum != 0 ||
        d->axes[ABS_MT_POSITION_Y].minimum != 0 || d->axes[ABS_MT_TRACKING_ID].minimum != 0 ||
        c->slots < 2 || c->slots > 32 || c->max_id == INT_MAX || c->max_id < c->slots) {
        errno = ENOTSUP; return failure("unsupported_mt_ranges");
    }
    if ((bit(d->abs_bits, ABS_MT_TOOL_X) &&
         (d->axes[ABS_MT_TOOL_X].minimum != 0 || d->axes[ABS_MT_TOOL_X].maximum != c->max_x)) ||
        (bit(d->abs_bits, ABS_MT_TOOL_Y) &&
         (d->axes[ABS_MT_TOOL_Y].minimum != 0 || d->axes[ABS_MT_TOOL_Y].maximum != c->max_y))) {
        errno = ENOTSUP; return failure("unsupported_tool_coordinate_ranges");
    }
    return 0;
}

int ph_open(const char *path, struct caps *c, struct device **result) {
    struct device *d = calloc(1, sizeof(*d));
    if (!d) return failure("allocate_device");
    d->physical = d->virtual = -1;
    d->physical = open(path, O_RDONLY | O_CLOEXEC | O_NONBLOCK | O_NOFOLLOW);
    int status = d->physical < 0 ? failure("open_physical_readonly") : read_caps(d, c);
    if (status < 0) { ph_close(d); return status; }
    *result = d;
    return 0;
}

int ph_create(struct device *d) {
    d->virtual = open("/dev/uinput", O_WRONLY | O_CLOEXEC | O_NONBLOCK);
    CHECK(d->virtual);
    CHECK(ioctl(d->virtual, UI_SET_EVBIT, EV_SYN));
    CHECK(ioctl(d->virtual, UI_SET_EVBIT, EV_KEY));
    for (int i = 0; i < KEY_CNT; ++i) if (bit(d->keys, i)) CHECK(ioctl(d->virtual, UI_SET_KEYBIT, i));
    CHECK(ioctl(d->virtual, UI_SET_EVBIT, EV_ABS));
    for (int i = 0; i < ABS_CNT; ++i) {
        if (!bit(d->abs_bits, i)) continue;
        CHECK(ioctl(d->virtual, UI_SET_ABSBIT, i));
        struct uinput_abs_setup setup = { .code = (uint16_t)i, .absinfo = d->axes[i] };
        setup.absinfo.value = setup.absinfo.minimum > 0 ? setup.absinfo.minimum :
                              (setup.absinfo.maximum < 0 ? setup.absinfo.maximum : 0);
        if (i == ABS_MT_SLOT) ++setup.absinfo.maximum;
        CHECK(ioctl(d->virtual, UI_ABS_SETUP, &setup));
    }
    for (int i = 0; i < MSC_CNT; ++i) {
        if (!bit(d->msc_bits, i)) continue;
        CHECK(ioctl(d->virtual, UI_SET_EVBIT, EV_MSC));
        CHECK(ioctl(d->virtual, UI_SET_MSCBIT, i));
    }
    for (int i = 0; i < INPUT_PROP_CNT; ++i) if (bit(d->properties, i)) CHECK(ioctl(d->virtual, UI_SET_PROPBIT, i));
    struct uinput_setup setup = { .id = { .bustype = BUS_VIRTUAL, .vendor = 0x5041, .product = 1, .version = 1 } };
    snprintf(setup.name, sizeof(setup.name), "Paimon proxy touchscreen");
    CHECK(ioctl(d->virtual, UI_DEV_SETUP, &setup));
    CHECK(ioctl(d->virtual, UI_DEV_CREATE));
    d->created = 1;
    return 0;
}

static int idle_state(struct device *d, const struct caps *c) {
    int32_t slots[33] = { ABS_MT_TRACKING_ID };
    CHECK(ioctl(d->physical, EVIOCGMTSLOTS((c->slots + 1) * sizeof(int32_t)), slots));
    for (int i = 1; i <= c->slots; ++i) if (slots[i] != -1) return 0;
    unsigned char keys[(KEY_CNT + 7) / 8] = {0};
    CHECK(ioctl(d->physical, EVIOCGKEY(sizeof(keys)), keys));
    return bit(keys, BTN_TOUCH) || (c->tool_finger && bit(keys, BTN_TOOL_FINGER)) ? 0 : 1;
}

static int all_up(struct device *d, const struct caps *c) {
    int idle = idle_state(d, c);
    if (idle < 0) return idle;
    if (!idle) { errno = EBUSY; return failure("fingers_must_be_up"); }
    return 0;
}

int ph_grab(struct device *d, struct caps *c) {
    CHECK(all_up(d, c));
    CHECK(ioctl(d->physical, EVIOCGRAB, 1));
    d->grabbed = 1;
    /* Discard pre-takeover history, then verify a complete idle snapshot under grab. */
    struct input_event events[64];
    for (int batch = 0; ; ++batch) {
        ssize_t count = read(d->physical, events, sizeof(events));
        if (count < 0 && errno == EAGAIN) break;
        if (count <= 0) return failure("drain_physical");
        if (batch >= 64) { errno = EBUSY; return failure("source_not_idle"); }
    }
    CHECK(all_up(d, c));
    struct input_absinfo slot;
    CHECK(ioctl(d->physical, EVIOCGABS(ABS_MT_SLOT), &slot));
    c->selected = slot.value;
    return 0;
}

int ph_write(struct device *d, const struct event *events, int count);

/* Restore every inactive physical slot's cached axes. A driver may omit X/Y
   on the next DOWN when they equal the previous contact's coordinates. */
int ph_seed(struct device *d, const struct caps *c) {
    for (int axis = ABS_MT_TOUCH_MAJOR; axis <= ABS_MT_TOOL_Y; ++axis) {
        if (axis == ABS_MT_TRACKING_ID || !bit(d->abs_bits, axis)) continue;
        int32_t values[33] = {0};
        values[0] = axis;
        CHECK(ioctl(d->physical, EVIOCGMTSLOTS((c->slots + 1) * sizeof(int32_t)), values));
        for (int slot = 0; slot < c->slots; ++slot) {
            struct event events[] = {{EV_ABS, ABS_MT_SLOT, slot}, {EV_ABS, (uint16_t)axis, values[slot + 1]}};
            CHECK(ph_write(d, events, 2));
        }
    }
    struct event finish[] = {{EV_ABS, ABS_MT_SLOT, c->selected}, {EV_SYN, SYN_REPORT, 0}};
    CHECK(ph_write(d, finish, 2));
    /* Do not start from a snapshot which raced with a real gesture. Any event
       queued during snapshot/seed invalidates this handover; require a retry. */
    struct input_event pending;
    ssize_t count = read(d->physical, &pending, sizeof(pending));
    if (count < 0 && errno == EAGAIN) return 0;
    errno = EBUSY;
    return failure("touch_during_handover");
}

void ph_describe(struct device *d) {
    printf("{\"event\":\"device_metadata\",\"properties\":[");
    const char *separator = "";
    for (int i = 0; i < INPUT_PROP_CNT; ++i) if (bit(d->properties, i)) { printf("%s%d", separator, i); separator = ","; }
    printf("],\"keys\":["); separator = "";
    for (int i = 0; i < KEY_CNT; ++i) if (bit(d->keys, i)) { printf("%s%d", separator, i); separator = ","; }
    printf("],\"axes\":["); separator = "";
    for (int i = 0; i < ABS_CNT; ++i) if (bit(d->abs_bits, i)) {
        struct input_absinfo a = d->axes[i];
        printf("%s{\"code\":%d,\"minimum\":%d,\"maximum\":%d,\"resolution\":%d,\"fuzz\":%d,\"flat\":%d}",
               separator, i, a.minimum, a.maximum, a.resolution, a.fuzz, a.flat);
        separator = ",";
    }
    printf("]}\n");
    fflush(stdout);
}

int ph_poll(struct device *d, int watch_stdin) {
    struct pollfd fds[2] = {{d->physical, POLLIN, 0}, {watch_stdin ? STDIN_FILENO : -1, POLLIN, 0}};
    int ready = poll(fds, 2, 10);
    if (ready < 0 && errno == EINTR) ready = 0;
    CHECK(ready);
    if (fds[0].revents & (POLLERR | POLLHUP | POLLNVAL)) { errno = ENODEV; return failure("physical_disconnected"); }
    return (stopping ? 4 : 0) | (fds[0].revents & POLLIN ? 1 : 0) | (fds[1].revents & (POLLIN | POLLHUP | POLLERR) ? 2 : 0);
}

int ph_handover_ready(struct device *d, const struct caps *c) {
    /* A userspace budget ending at SYN_REPORT is not proof the kernel queue is
       drained. Check both queue and live slots, then check for a raced frame. */
    for (int pass = 0; pass < 2; ++pass) {
        struct pollfd fd = {d->physical, POLLIN, 0};
        int ready = poll(&fd, 1, 0);
        if (ready < 0 && errno == EINTR) return 0;
        CHECK(ready);
        if (fd.revents & (POLLERR | POLLHUP | POLLNVAL)) { errno = ENODEV; return failure("handover_device_lost"); }
        if (ready) return 0;
        if (pass == 0) {
            int idle = idle_state(d, c);
            if (idle <= 0) return idle;
        }
    }
    return 1;
}

int ph_read(struct device *d, struct event *out) {
    struct input_event input;
    ssize_t count = read(d->physical, &input, sizeof(input));
    if (count < 0 && (errno == EAGAIN || errno == EINTR)) return 0;
    if (count != sizeof(input)) return failure("read_input_event");
    *out = (struct event) {input.type, input.code, input.value};
    return 1;
}

int ph_write(struct device *d, const struct event *events, int count) {
    for (int i = 0; i < count; ++i) {
        struct input_event output = { .type = events[i].kind, .code = events[i].code, .value = events[i].value };
        ssize_t written;
        do { written = write(d->virtual, &output, sizeof(output)); } while (written < 0 && errno == EINTR);
        if (written != sizeof(output)) return failure("write_uinput_event");
    }
    return 0;
}

int ph_stdin(unsigned char *buffer, int capacity) {
    ssize_t count = read(STDIN_FILENO, buffer, (size_t)capacity);
    if (count < 0 && (errno == EAGAIN || errno == EINTR)) return -EAGAIN;
    if (count < 0) return failure("read_commands");
    return (int)count;
}
