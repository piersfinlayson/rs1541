debugging findings:
- add cbm_reset to get_status_alredy_locked
- then retrieved status and printed
- but seemsed to go into an error handling mode - seemed to do open write unlisten in that order - in dir code?
- may be that the BusMode support is bugged - I think open write unlisten ok - open but check opencbm code

So at least two problems
- Why did I need to reset the bus?  I have done a device status read first - talk, read, untalk in that order

*** need to look at read return codes - and deal with 0 or fewer bytes than expected

want better error mapping (prob need better CbmErrors all in)