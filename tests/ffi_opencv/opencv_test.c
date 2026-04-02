/* OpenCV FFI verification test.
 * Creates a 64x64 image, draws a rectangle, reads a pixel.
 * Links against libopencv_core + libopencv_imgproc.
 *
 * Build: gcc -o opencv_test opencv_test.c $(pkg-config --cflags --libs opencv4) -lstdc++
 * Run:   ./opencv_test
 */
#include <stdio.h>
#include <opencv2/core/core_c.h>
#include <opencv2/imgproc/imgproc_c.h>

int main(void) {
    /* Create a 64x64 single-channel 8-bit image, filled with zeros */
    IplImage *img = cvCreateImage(cvSize(64, 64), IPL_DEPTH_8U, 1);
    cvZero(img);

    /* Draw a white rectangle (255) from (10,10) to (50,50) */
    cvRectangle(img,
                cvPoint(10, 10),
                cvPoint(50, 50),
                cvScalar(255, 0, 0, 0),
                -1,  /* filled */
                8, 0);

    /* Read pixel at (30,30) — should be inside the rectangle (255) */
    CvScalar pixel = cvGet2D(img, 30, 30);
    int value = (int)pixel.val[0];

    /* Read pixel at (5,5) — should be outside the rectangle (0) */
    CvScalar outside = cvGet2D(img, 5, 5);
    int outside_value = (int)outside.val[0];

    printf("OPENCV-FFI-OK pixel_inside=%d pixel_outside=%d\n", value, outside_value);

    cvReleaseImage(&img);

    /* Verify correctness */
    if (value == 255 && outside_value == 0) {
        printf("PASS: OpenCV image processing verified\n");
        return 0;
    } else {
        printf("FAIL: unexpected pixel values\n");
        return 1;
    }
}
