DROP INDEX threads_thread_number_idx;
CREATE UNIQUE INDEX threads_thread_number_idx ON threads(thread_number);
