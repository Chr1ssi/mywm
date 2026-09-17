#include <gtk/gtk.h>
#include <stdio.h>

static GtkWidget *window;
static const char *control;
static const char *status;
static int previous = -1;

static gboolean draw(GtkWidget *widget, cairo_t *cr, gpointer data) {
    (void)widget; (void)data;
    cairo_set_source_rgb(cr, 1, 0, 0);
    cairo_paint(cr);
    return FALSE;
}

static gboolean tick(gpointer data) {
    (void)data;
    FILE *f = fopen(control, "r");
    int request = f ? fgetc(f) : '0';
    if (f) fclose(f);
    if (request != previous) {
        if (request == '1') gtk_window_fullscreen(GTK_WINDOW(window));
        else gtk_window_unfullscreen(GTK_WINDOW(window));
        previous = request;
    }
    int width, height;
    gtk_window_get_size(GTK_WINDOW(window), &width, &height);
    GdkWindow *native = gtk_widget_get_window(window);
    f = fopen(status, "w");
    if (f) {
        fprintf(f, "%d %d %d\n", width, height,
                !!(gdk_window_get_state(native) & GDK_WINDOW_STATE_FULLSCREEN));
        fclose(f);
    }
    return G_SOURCE_CONTINUE;
}

int main(int argc, char **argv) {
    gtk_init(&argc, &argv);
    if (argc != 3) return 2;
    control = argv[1]; status = argv[2];
    window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_decorated(GTK_WINDOW(window), FALSE);
    GtkWidget *area = gtk_drawing_area_new();
    gtk_container_add(GTK_CONTAINER(window), area);
    g_signal_connect(area, "draw", G_CALLBACK(draw), NULL);
    g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
    gtk_widget_show_all(window);
    g_timeout_add(50, tick, NULL);
    gtk_main();
    return 0;
}
