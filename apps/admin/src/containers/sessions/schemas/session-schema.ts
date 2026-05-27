import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const startSessionSchema = yup.object({
  playerId: yup
    .string()
    .uuid('Please select a valid player')
    .required(validationMessages.required('Player')),

  playerPlanId: yup
    .string()
    .uuid('Please select a valid player plan')
    .required(validationMessages.required('Player Plan')),

  deviceId: yup
    .string()
    .uuid('Please select a valid device')
    .required(validationMessages.required('Device')),

  startTime: yup.string().optional().nullable(),
});

export type StartSessionFormData = yup.InferType<typeof startSessionSchema>;

export const startSessionDefaultValues: StartSessionFormData = {
  playerId: '',
  playerPlanId: '',
  deviceId: '',
  startTime: undefined,
};

export const endSessionSchema = yup.object({
  endTime: yup.string().optional().nullable(),
});

export type EndSessionFormData = yup.InferType<typeof endSessionSchema>;

export const endSessionDefaultValues: EndSessionFormData = {
  endTime: undefined,
};
