// ts-unused-exports:disable-next-line
export default (): void => {
  process.env.PGDATABASE = 'postgres';
  process.env.PGPASSWORD = 'postgres';
  process.env.PGUSER = 'test';
  process.env.PGHOST = 'localhost';
};
