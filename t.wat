(module
  ;; Import 1 page (64Kib) of shared memory. 
  (import "env" "memory" (memory (shared 1 1)))
 
  ;; Try to lock a mutex at the given address.
  ;; Returns 1 if the mutex was successfully locked, and 0 otherwise.
  (func $tryLockMutex (export "tryLockMutex")
    (param $mutexAddr i32) (result i32)
    ;; Attempt to grab the mutex. The cmpxchg operation atomically
    ;; does the following:
    ;; - Loads the value at $mutexAddr.
    ;; - If it is 0 (unlocked), set it to 1 (locked).
    ;; - Return the originally loaded value.
    (i32.atomic.rmw.cmpxchg
      (get_local $mutexAddr) ;; mutex address
      (i32.const 0)          ;; expected value (0 => unlocked)
      (i32.const 1))         ;; replacement value (1 => locked)
      
    ;; The top of the stack is the originally loaded value.
    ;; If it is 0, this means we acquired the mutex. We want to
    ;; return the inverse (1 means mutex acquired), so use i32.eqz
    ;; as a logical not.
    (i32.eqz)
  )
  
  ;; Lock a mutex at the given address, retrying until successful.
  (func (export "lockMutex")
    (param $mutexAddr i32)
    (block $done
      (loop $retry
        ;; Try to lock the mutex. $tryLockMutex returns 1 if the mutex
        ;; was locked, and 0 otherwise.
        (call $tryLockMutex (get_local $mutexAddr))
        (br_if $done)
        
        ;; Wait for the other agent to finish with mutex.
        (i32.wait
          (get_local $mutexAddr) ;; mutex address
          (i32.const 1)          ;; expected value (1 => locked)
          (i64.const -1))        ;; infinite timeout
(drop)        
        ;; Try to acquire the lock again.
        (br $retry)
      )
    )
  )
  
  ;; Unlock a mutex at the given address.
  (func (export "unlockMutex")
    (param $mutexAddr i32)
    ;; Unlock the mutex.
    (i32.atomic.store
      (get_local $mutexAddr)     ;; mutex address
      (i32.const 0))             ;; 0 => unlocked
 
    ;; Wake one agent that is waiting on this lock.
    (wake
      (get_local $mutexAddr)     ;; mutex address
      (i32.const 1))             ;; wake 1 waiter
(drop)
  )
)
