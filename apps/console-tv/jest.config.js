module.exports = {
  testMatch: ['<rootDir>/__tests__/hudTimerUtils.test.ts'],
  testEnvironment: 'node',
  transform: {
    '^.+\\.[tj]sx?$': 'babel-jest',
  },
};
