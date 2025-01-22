*** need to look at read return codes - and deal with 0 or fewer bytes than expected
 - have fixed read_drive_memory and read_from_drive
also should pass in mut bufs to all read functions

Make memory_read/write functions handle both DOS 1 and DOS 2.  The cutoff where it's worthwhile to see if the device is DOS 1 is around 4-8 bytes (as 5 bytes need to be written just to read/write each 1 with DOS 1 mechanism)

Test my 2031 and 1540 differentiation code

Deal with device/bus timeouts properly