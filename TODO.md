debugging findings:
- add cbm_reset to get_status_alredy_locked
- then retrieved status and printed
- but seemsed to go into an error handling mode - seemed to do open write unlisten in that order - in dir code?
- may be that the BusMode support is bugged - I think open write unlisten ok - open but check opencbm code

So at least two problems
- Why did I need to reset the bus?  I have done a device status read first - talk, read, untalk in that order

*** need to look at read return codes - and deal with 0 or fewer bytes than expected
 - have fixed read_drive_memory and read_from_drive
also should pass in mut bufs to all read functions

Make memory_read/write functions handle both DOS 1 and DOS 2.  The cutoff where it's worthwhile to see if the device is DOS 1 is around 4-8 bytes (as 5 bytes need to be written just to read/write each 1 with DOS 1 mechanism)